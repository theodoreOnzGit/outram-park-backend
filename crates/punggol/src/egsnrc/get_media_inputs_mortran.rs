// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/get_media_inputs.mortran (with
//                     pegs4_macros.mortran:599-641 and pegs4_routines.mortran:2280)
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada (author Blake
//                     Walters 2013; contributors Reid Townson, Frederic Tessier)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/get_media_inputs.mortran` (EGSnrc): the media-input reader
//! of pegsless mode, `get_media_inputs`.
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit
//! `bd49a47aae3938f49f4100b8e018091861408b19` (read 2026-11-21, a shallow
//! clone, never committed here). Licence: AGPL-3.0.
//!
//! **Status: first port, not differentially tested (gh:#1680).**
//!
//! | upstream | here |
//! |---|---|
//! | `SUBROUTINE GET_INPUT_PLUS` (`get_media_inputs.mortran:32-632`) | [`get_input_plus`](super::get_inputs_mortran::get_input_plus), in `get_inputs_mortran.rs` (shares one engine with `GET_INPUT`) |
//! | `subroutine get_media_inputs` (`:634-1886`) | [`get_media_inputs`] |
//! | `DATA ASYMT`, `DATA WATBL`, `DATA NET` (`pegs4_macros.mortran:599-641`) | [`ASYMT`], [`WATBL`], [`NET`] |
//! | `FUNCTION ZTBL` (`pegs4_routines.mortran:2280-2300`) | [`ztbl`] |
//!
//! `ASYMT`, `WATBL` and `ztbl` belong to the PEGS4 placeholders
//! (`pegs4_macros_mortran.rs`, `pegs4_routines_mortran.rs`); they are here
//! only because this routine needs them and those files were not mine to edit.
//! Move them when those files are ported.
//!
//! # Not ported
//!
//! * ~~`egs_compoundstoelements` (called at `:1720`): its source is not in the
//!   EGSnrc tree that was read, so a `mixture compounds` medium is not
//!   expanded to elements~~ **CORRECTED 2026-11-21 (gh:#1691):** it is in
//!   `HEN_HOUSE/egs++/estar/estar.cpp` and is ported in [`super::estar_cpp`];
//!   a `mixture compounds` medium is now expanded (ascending `Z`, mass
//!   fractions normalised). The compound names must be chemical formulas
//!   (`H2O`), as upstream requires.
//! * The `entry show_media_parameters` (`:1888-2080`): output formatting, and
//!   the stopping-power plot it drives (`EDEDX`, `MEDXVGRPLOT`, `:1955+`),
//!   which the task excluded.
//! * File and unit I/O: `egs_open_file`, `egs_get_unit`, `open`, `inquire`.
//!   The deck, the material data file and the density correction files come in
//!   as text ([`MediaFile`]). The density-file directory search (`$EGS_HOME`,
//!   `$HEN_HOUSE`, `:1530-1595`) is replaced by a lookup of the file name in
//!   the supplied list (see [`get_media_inputs`]).
//! * `write(*,*)' density_file '` (`:1531`, a stray debug print to stdout).
//!
//! # Where the result goes
//!
//! Upstream stores into `COMMON/MEDIA`, `ELECIN`, `THRESH`, `MEDINP`. Here
//! [`MediaInputs`] holds the same data, reusing [`Pegs4Element`] for the
//! element rows. `Pegs4Medium` (the PEGS4 data set, with cross-section
//! tables) is not built here: pegsless mode computes those later from this
//! composition.
//!
//! # Things in the upstream source that look wrong
//!
//! See the report on gh:#1680; each is marked `UPSTREAM?` below.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_mortran::{Pegs4Element, RM};
use super::get_inputs_mortran::{
    eq_blank_padded, fixed, get_input, get_input_plus, index1, lossy, read_list, skip_leading_blanks, tail, trim_blanks, upper,
    GetInputCommon, InputType, InputUnit, MAX_VALUES,
};

/// `NET` (`pegs4_macros.mortran:599`): the number of elements in the tables.
pub const NET: usize = 100;

/// `ASYMT` (`pegs4_macros.mortran:633-641`): element symbols, `Z = 1..100`.
pub const ASYMT: [&str; NET] = [
    "H", "HE", "LI", "BE", "B", "C", "N", "O", "F", "NE",
    "NA", "MG", "AL", "SI", "P", "S", "CL", "AR", "K", "CA",
    "SC", "TI", "V", "CR", "MN", "FE", "CO", "NI", "CU", "ZN",
    "GA", "GE", "AS", "SE", "BR", "KR", "RB", "SR", "Y", "ZR",
    "NB", "MO", "TC", "RU", "RH", "PD", "AG", "CD", "IN", "SN",
    "SB", "TE", "I", "XE", "CS", "BA", "LA", "CE", "PR", "ND",
    "PM", "SM", "EU", "GD", "TB", "DY", "HO", "ER", "TM", "YB",
    "LU", "HF", "TA", "W", "RE", "OS", "IR", "PT", "AU", "HG",
    "TL", "PB", "BI", "PO", "AT", "RN", "FR", "RA", "AC", "TH",
    "PA", "U", "NP", "PU", "AM", "CM", "BK", "CF", "ES", "FM",
];

/// `WATBL` (`pegs4_macros.mortran:610-621`): atomic weights, `REAL*4`
/// (`$REAL4 WATBL`), so the values are the single-precision constants.
pub const WATBL: [f32; NET] = [
    1.00797_f32, 4.0026_f32, 6.939_f32, 9.0122_f32, 10.811_f32, 12.01115_f32, 14.0067_f32,
    15.9994_f32, 18.9984_f32, 20.183_f32, 22.9898_f32, 24.312_f32, 26.9815_f32, 28.088_f32,
    30.9738_f32, 32.064_f32, 35.453_f32, 39.948_f32, 39.102_f32, 40.08_f32, 44.956_f32,
    47.90_f32, 50.942_f32, 51.998_f32, 54.9380_f32, 55.847_f32, 58.9332_f32, 58.71_f32,
    63.54_f32, 65.37_f32, 69.72_f32, 72.59_f32, 74.9216_f32, 78.96_f32, 79.808_f32,
    83.80_f32, 85.47_f32, 87.62_f32, 88.905_f32, 91.22_f32, 92.906_f32, 95.94_f32,
    99.0_f32, 101.07_f32, 102.905_f32, 106.4_f32, 107.87_f32, 112.4_f32, 114.82_f32,
    118.69_f32, 121.75_f32, 127.60_f32, 126.9044_f32, 131.30_f32, 132.905_f32, 137.34_f32,
    138.91_f32, 140.12_f32, 140.907_f32, 144.24_f32, 147.0_f32, 150.35_f32, 151.98_f32,
    157.25_f32, 158.924_f32, 162.50_f32, 164.930_f32, 167.26_f32, 168.934_f32, 173.04_f32,
    174.97_f32, 178.49_f32, 180.948_f32, 183.85_f32, 186.2_f32, 190.2_f32, 192.2_f32,
    195.08_f32, 196.987_f32, 200.59_f32, 204.37_f32, 207.19_f32, 208.980_f32, 210.0_f32,
    210.0_f32, 222.0_f32, 223.0_f32, 226.0_f32, 227.0_f32, 232.036_f32, 231.0_f32,
    238.03_f32, 237.0_f32, 242.0_f32, 243.0_f32, 247.0_f32, 247.0_f32, 248.0_f32,
    254.0_f32, 253.0_f32, ];

/// `ZTBL` (`pegs4_routines.mortran:2280-2300`): the atomic number of a
/// symbol, `A` being the alternate symbol for argon. `IASYM` is `$TYPE`
/// (`CHARACTER*4`), so only the first four characters of `sym` count, and the
/// comparison is case sensitive. `None` where upstream prints "not an atomic
/// symbol" and returns `0.0` (which the callers then use as an index into
/// `WATBL`, out of bounds; this port reports it instead).
pub fn ztbl(sym: &str) -> Option<usize> {
    let b = fixed(sym.as_bytes(), 4);
    let s = trim_blanks(&b);
    if s == b"A" {
        return Some(18);
    }
    ASYMT.iter().position(|a| a.as_bytes() == s).map(|p| p + 1)
}

/// `WATBL(z)` widened to `REAL*8`, as `wa(i,j)=WATBL(i01)` does.
fn watbl(z: usize) -> f64 {
    f64::from(WATBL[z - 1])
}

/// A text file handed in place of a file on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaFile {
    /// The name the deck refers to it by.
    pub name: String,
    /// Its contents.
    pub text: String,
}

/// What `get_media_inputs` takes from the rest of the simulation, and the
/// array bounds that are compile-time macros upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInputConfig {
    /// `$MXMED`.
    pub max_media: usize,
    /// `$MXEL`.
    pub max_elements: usize,
    /// `prm`, MeV: only used for the `ue` default `50 + prm` (`:781`).
    pub prm: f64,
    /// `n_parallel` (`COMMON/EGS-IO`): the `.mederr` messages and the output
    /// density file are only produced when it is 0 or `i_parallel` is
    /// `first_parallel`.
    pub n_parallel: i32,
    /// `i_parallel`.
    pub i_parallel: i32,
    /// `first_parallel`.
    pub first_parallel: i32,
}

impl MediaInputConfig {
    /// Non-parallel run (`n_parallel = 0`) with `prm` = [`RM`].
    pub fn new(max_media: usize, max_elements: usize) -> Self {
        Self { max_media, max_elements, prm: RM, n_parallel: 0, i_parallel: 0, first_parallel: 0 }
    }

    fn is_first(&self) -> bool {
        self.n_parallel == 0 || self.i_parallel == self.first_parallel
    }
}

/// One element row as stored in `COMMON/MEDIA` and `MEDINP`: the
/// [`Pegs4Element`] (`inpasym`, `zelem`, `wa`, `pz`, `rhoz`) and the
/// `REAL*4` twins `pz4`, `rhoz4`, `wa4`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediumElement {
    /// `inpasym`, `zelem`, `wa`, `pz`, `rhoz`.
    pub element: Pegs4Element,
    /// `pz4(i,j)` (`REAL*4`).
    pub pz4: f32,
    /// `rhoz4(i,j)` (`REAL*4`).
    pub rhoz4: f32,
    /// `wa4(i,j)` (`REAL*4`).
    pub wa4: f32,
}

/// Everything `get_media_inputs` commits for one medium (`:1648-1760`).
/// When `committed` is false the medium was "not correctly defined"
/// (`:1769`) and every other field is at its zero value, as the arrays are
/// upstream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediumInput {
    /// `media(1:24,i)` without trailing blanks.
    pub name: String,
    /// `elements_specified & rho_specified` held.
    pub committed: bool,
    /// `ae(i)`, MeV.
    pub ae: f64,
    /// `ue(i)`, MeV.
    pub ue: f64,
    /// `ap(i)`, MeV.
    pub ap: f64,
    /// `up(i)`, MeV.
    pub up: f64,
    /// `inpstrn(1:24,i)`: the Sternheimer id, default the medium name.
    pub sterncid: String,
    /// `nne(i)`. 0 for a mixture that still needs expanding.
    pub nne: usize,
    /// `rho(i)`, g/cm^3 (bulk density).
    pub rho: f64,
    /// `localDensity(i)` (`REAL*4`).
    pub local_density: f32,
    /// `ipot(i)` (`REAL*4`): the I-value, -1 if not given.
    pub ipot: f32,
    /// `iscompound(i)`: 1 if given by numbers of atoms with more than one element.
    pub iscompound: i32,
    /// The element rows.
    pub elements: Vec<MediumElement>,
    /// `ncomp(i)`: number of compounds of a mixture.
    pub ncomp: usize,
    /// `compoundList(i,j)`.
    pub compound_list: Vec<String>,
    /// `compoundFractions(i,j)` (`REAL*4`).
    pub compound_fractions: Vec<f32>,
    /// ~~True for a `mixture compounds` medium: `egs_compoundstoelements` is
    /// not ported~~ **CORRECTED 2026-11-21:** always false now, the mixture is
    /// expanded (kept for callers; gh:#1691).
    pub needs_compound_expansion: bool,
    /// `iunrst(i)`.
    pub iunrst: i32,
    /// `iaprim(i)`.
    pub iaprim: i32,
    /// `epstfl(i)`: 1 if a density correction file supplied the data.
    pub epstfl: i32,
    /// `inpgasp(i)` (`REAL*4`).
    pub inpgasp: f32,
    /// `inpdensity_file(i)`: blank (empty) if none.
    pub inpdensity_file: String,
    /// `outdensity_file(i)`, if requested (and this is the first parallel job).
    pub outdensity_file: Option<String>,
    /// `spoutput_file(i)`: the stopping-power plot file requested, if any.
    pub spoutput_file: Option<String>,
    /// `df_if_elem_mismatch(i)`.
    pub df_if_elem_mismatch: bool,
    /// `df_if_rho_mismatch(i)`.
    pub df_if_rho_mismatch: bool,
}

/// The result of [`get_media_inputs`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaInputs {
    /// `material_file` if `medfile_specified`.
    pub material_data_file: Option<String>,
    /// The global `ae`, `ue`, `ap`, `up` (`ae_tmp`...), MeV.
    pub ae: f64,
    /// See `ae`.
    pub ue: f64,
    /// See `ae`.
    pub ap: f64,
    /// See `ae`.
    pub up: f64,
    /// One per medium, in the order of the names given.
    pub media: Vec<MediumInput>,
    /// What upstream writes to the `.mederr` file (empty when this is not the
    /// first parallel job). List-directed spacing is not reproduced.
    pub mederr: Vec<String>,
}

/// `ecut_min`/`pcut_min` (`:667-672`): the smallest of the region cuts,
/// starting from 999.
pub fn min_cuts(ecut: &[f64], pcut: &[f64]) -> (f64, f64) {
    let mut e = 999.0;
    let mut p = 999.0;
    for &x in ecut {
        if x < e {
            e = x;
        }
    }
    for &x in pcut {
        if x < p {
            p = x;
        }
    }
    (e, p)
}

/// The text files and the two input units, with the delimiters of the
/// material-data-file medium in use.
struct Units {
    egsinp: InputUnit,
    medfile: InputUnit,
    delim_start: String,
    delim_end: String,
}

impl Units {
    /// `CALL GET_INPUT` (`plus = false`) or `CALL GET_INPUT_PLUS(i_medfile,...)`.
    fn call(&mut self, plus: bool, c: &mut GetInputCommon) -> Result<(), String> {
        if plus {
            get_input_plus(&mut self.medfile, c, &self.delim_start, &self.delim_end)
        } else {
            get_input(&mut self.egsinp, c)
        }
    }
}

/// `$WRITE_MEDERR`.
struct Log {
    emit: bool,
    lines: Vec<String>,
}

impl Log {
    fn w(&mut self, s: &str) {
        if self.emit {
            self.lines.push(String::from(s));
        }
    }
}

/// The `*_tmp` variables of `get_media_inputs` (`:683-714`). They live for the
/// whole routine, not per medium, so a stale value survives into the next
/// medium exactly as upstream (it is only read when its `*_specified` flag
/// is set again).
struct Tmp {
    nne: usize,
    asym: Vec<String>,
    pz: Vec<f64>,
    rhoz: Vec<f64>,
    z: Vec<f64>,
    compound_list: Vec<String>,
    compound_fractions: Vec<f64>,
    rho: f64,
    localdensity: f64,
    ipot: f64,
    gasp: f64,
    sterncid: String,
    iunrst: i32,
    iaprim: i32,
    epstfl: i32,
    density_file: String,
    density_outputfile: String,
    elements_specified: bool,
    mixture_specified: bool,
    rho_specified: bool,
    spec_by_pz: bool,
    spec_by_rhoz: bool,
    stern_specified: bool,
    iunrst_specified: bool,
    iaprim_specified: bool,
    gasp_specified: bool,
    densityfile_specified: bool,
}

/// `char*N` assignment of a read string.
fn cut(s: String, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    String::from(t.trim_end_matches(' '))
}

/// The `elements` / `number of atoms` / `mass fractions` reads, which are the
/// same in the deck (`:870-936`, `plus = false`, element symbols upper-cased
/// by `$CONVERT`) and the material data file (`:1281-1344`, `plus = true`,
/// **not** upper-cased).
/// UPSTREAM? A lower-case symbol in a material data file is never converted,
/// and `ZTBL` is case sensitive (`:1288` against `:877`).
fn read_elements(c: &mut GetInputCommon, u: &mut Units, plus: bool, ival: &mut usize, t: &mut Tmp, max_el: usize, log: &mut Log) -> Result<(), String> {
    *ival += 1;
    let ie = *ival;
    c.declare(ie, "elements", InputType::Text, 0);
    c.seek(ie);
    u.call(plus, c)?;
    if c.error_flags[ie] != 0 {
        return Ok(());
    }
    if !plus {
        for j in 1..=c.nvalue[ie] {
            c.upper_char_value(ie, j);
        }
    }
    *ival += 1;
    let ipz = *ival;
    t.nne = c.nvalue[ie];
    if t.nne > max_el || t.nne > MAX_VALUES {
        return Err(format!("{} elements given; $MXEL is {}", t.nne, max_el));
    }
    c.declare(ipz, "number of atoms", InputType::Integer, t.nne);
    c.seek(ipz);
    u.call(plus, c)?;
    if t.nne > 1 && c.error_flags[ipz] == 0 {
        for j in 1..=t.nne {
            t.asym[j - 1] = cut(c.char_value(ie, j), 50);
            t.pz[j - 1] = c.value(ipz, j);
        }
        t.elements_specified = true;
        t.spec_by_pz = true;
    } else {
        *ival += 1;
        let irz = *ival;
        c.declare(irz, "mass fractions", InputType::Real, t.nne);
        c.seek(irz);
        if t.nne == 1 {
            c.set_limits(irz, 0.0, 1.0e15, 1.0);
        }
        u.call(plus, c)?;
        if c.error_flags[irz] == 0 {
            for j in 1..=t.nne {
                t.asym[j - 1] = cut(c.char_value(ie, j), 50);
                t.rhoz[j - 1] = c.value(irz, j);
            }
            t.elements_specified = true;
            t.spec_by_rhoz = true;
        }
    }
    if t.nne == 1 && !t.elements_specified {
        t.asym[0] = cut(c.char_value(ie, 1), 50);
        t.pz[0] = 1.0;
        t.elements_specified = true;
        t.spec_by_pz = true;
    }
    if t.elements_specified {
        log.w(if plus { " Composition specified in material data file" } else { " Composition specified in .egsinp file." });
    }
    Ok(())
}

/// A single-slot real read: declare, call, and return the value if the flag is 0.
fn read_real(c: &mut GetInputCommon, u: &mut Units, plus: bool, ival: &mut usize, name: &str, min: f64, max: f64, dflt: f64) -> Result<Option<f64>, String> {
    *ival += 1;
    let s = *ival;
    c.declare(s, name, InputType::Real, 1);
    c.set_limits(s, min, max, dflt);
    c.seek(s);
    u.call(plus, c)?;
    Ok(if c.error_flags[s] == 0 { Some(c.value(s, 1)) } else { None })
}

/// A single-slot string read.
fn read_text(c: &mut GetInputCommon, u: &mut Units, plus: bool, ival: &mut usize, name: &str) -> Result<Option<String>, String> {
    *ival += 1;
    let s = *ival;
    c.declare(s, name, InputType::Text, 1);
    c.seek(s);
    u.call(plus, c)?;
    Ok(if c.error_flags[s] == 0 { Some(c.char_value(s, 1)) } else { None })
}

/// `stopping powers` (`:1095-1117`, `:1394-1415`).
fn read_iunrst(c: &mut GetInputCommon, u: &mut Units, plus: bool, ival: &mut usize) -> Result<Option<i32>, String> {
    *ival += 1;
    let s = *ival;
    c.declare(s, "stopping powers", InputType::Allowed, 1);
    c.set_allowed(s, 0, "restricted total");
    c.set_allowed(s, 1, "unrestricted collision");
    c.set_allowed(s, 2, "unrestricted collision and radiative");
    c.set_allowed(s, 3, "unrestricted collision and restricted radiative");
    c.set_allowed(s, 4, "restricted collision and unrestricted radiative");
    c.set_allowed(s, 5, "unrestricted radiative");
    c.seek(s);
    u.call(plus, c)?;
    Ok(if c.error_flags[s] == 0 { Some(c.value(s, 1) as i32) } else { None })
}

/// `bremsstrahlung correction` (`:1119-1133`, `:1417-1432`).
fn read_iaprim(c: &mut GetInputCommon, u: &mut Units, plus: bool, ival: &mut usize) -> Result<Option<i32>, String> {
    *ival += 1;
    let s = *ival;
    c.declare(s, "bremsstrahlung correction", InputType::Allowed, 1);
    c.set_allowed(s, 0, "KM");
    c.set_allowed(s, 1, "NRC");
    c.set_allowed(s, 2, "none");
    c.seek(s);
    u.call(plus, c)?;
    Ok(if c.error_flags[s] == 0 { Some(c.value(s, 1) as i32) } else { None })
}

/// The header scan of the material data file (`:1243-1274`): the delimiters
/// for `medium_name`, as `(start_found, delim_start, delim_end)`; `delim_end`
/// is `"NONE"` when the end of the file ends the entry. Records are read as
/// `A80` (`text_string` is `character*80`).
fn find_medium_delimiters(medfile: &mut InputUnit, medium_name: &str) -> (bool, String, String) {
    medfile.rewind();
    let (mut start_found, mut end_found) = (false, false);
    let (mut ds, mut de) = (String::new(), String::from("NONE"));
    while !start_found || !end_found {
        let Some(rec) = medfile.read() else { break };
        let save = fixed(&rec, 80);
        let mut up = save.clone();
        upper(&mut up);
        let mindex = index1(&up, b"MEDIUM");
        let eindex = index1(&up, b"=");
        if mindex > 0 && eindex > mindex {
            let mut ts = tail(&save, eindex + 1, 80);
            skip_leading_blanks(&mut ts);
            if eq_blank_padded(&ts, medium_name.as_bytes()) {
                ds = lossy(trim_blanks(&save));
                start_found = true;
            } else if start_found {
                de = lossy(trim_blanks(&save));
                end_found = true;
            }
        }
    }
    (start_found, ds, de)
}

/// The data of a density correction file (`:1608-1620`).
struct DensityFile {
    nne: usize,
    rho: f64,
    z: Vec<f64>,
    rhoz: Vec<f64>,
}

/// `read(i_density,'(a)')title; read(i_density,*)nepst_df,iev_df,rho_df,nne_df;
/// read(i_density,*)(z_df(j),rhoz_df(j),j=1,nne_df)`. A null item reads as 0
/// (the variable would be uninitialised upstream).
fn read_density_file(text: &str, max_el: usize) -> Result<DensityFile, String> {
    let mut u = InputUnit::from_text(text);
    u.read().ok_or_else(|| String::from("density correction file is empty"))?;
    let h = read_list(&mut u, &[true, false, false, true])?;
    let rho = h[2].unwrap_or(0.0);
    let n = h[3].unwrap_or(0.0);
    if n < 0.0 || n as usize > max_el {
        return Err(format!("density correction file has {} elements; $MXEL is {}", n, max_el));
    }
    let nne = n as usize;
    let mut z = Vec::new();
    let mut rhoz = Vec::new();
    if nne > 0 {
        let kinds = vec![false; 2 * nne];
        let v = read_list(&mut u, &kinds)?;
        for j in 0..nne {
            z.push(v[2 * j].unwrap_or(0.0));
            rhoz.push(v[2 * j + 1].unwrap_or(0.0));
        }
    }
    Ok(DensityFile { nne, rho, z, rhoz })
}

/// `subroutine get_media_inputs(ounit)` (`get_media_inputs.mortran:634-1886`):
/// read the `AE/UE/AP/UP` thresholds and, for each medium, its composition,
/// density and PEGS-less options from the input deck, falling back to the
/// material data file and a density correction file.
///
/// The deck is searched in the block `:start media definition:` /
/// `:stop media definition:` for `material data file`, `ae`, `ap`, `ue`,
/// `up`, and in each medium's own block `:start <NAME>:` / `:stop <NAME>:`
/// for `elements`, `number of atoms`, `mass fractions`, `mixture compounds`,
/// `rho`, `bulk density`, `local density`, `output density file`, `ivalue`,
/// `sterncid`, `stopping powers`, `bremsstrahlung correction`, `gas
/// pressure`, `density correction file` and `e- stopping power output file`.
/// What the deck leaves out is looked for in the medium's entry in the
/// material data file, which starts at the first line holding `MEDIUM`
/// before an `=` and whose text after the `=` is the medium name, and ends at
/// the next such line.
///
/// Defaults, as upstream: `ae` = smallest `ecut` (`:763`), `ap` = smallest
/// `pcut` (`:771`), `ue` = `50 + prm` (`:779`), `up` = 50 (`:787`), all in MeV
/// and accepted in `[0, 999]` (anything else becomes the default); `sterncid`
/// = the medium name (`:951`); `gas pressure` = 0 (`:957`); `stopping powers`
/// = 0 (`restricted total`, `:958`); `bremsstrahlung correction` = 1 (`NRC`,
/// `:959`); `ipot` = -1 if no `ivalue` (`:1670`); `epstfl` = 0 (`:960`);
/// `local density` = `rho` (`:1664`); `mass fractions` of a single element
/// default to 1 (`:923`). `ivalue` is read with default -2 and accepted in
/// `[0, 1e15]` (`:1065-1072`).
///
/// `density_files` stands for the directories upstream searches (`:1530-1595`):
/// the file is the entry whose `name` is the deck's `density correction file`
/// with `.density` appended, or the name unchanged when it contains a `/`.
///
/// `c` is the GetInput COMMON block; pass the one `get_transport_parameter`
/// left (see [`GetInputCommon::new`]). Its two `error_level`s are set to 0
/// (`:722-723`) and stay 0.
///
/// # Errors
///
/// Where upstream calls `$egs_fatal`: `UE <= AE`, `UP <= AP`, an unopenable
/// material data file or density correction file, `elements` together with
/// `mixture compounds`, `rho` with `bulk density`, `local density` without
/// `bulk density`; plus the reports this port adds where Fortran would index
/// out of bounds: more media than `max_media`, more elements than
/// `max_elements`, an unknown element symbol, a density-file `Z` outside
/// `1..=100`; plus the `Err`s of [`get_input`] and a malformed density file.
#[allow(clippy::too_many_arguments)]
pub fn get_media_inputs(
    egsinp: &str,
    medium_names: &[&str],
    ecut: &[f64],
    pcut: &[f64],
    material_files: &[MediaFile],
    density_files: &[MediaFile],
    cfg: &MediaInputConfig,
    c: &mut GetInputCommon,
) -> Result<MediaInputs, String> {
    let nmed = medium_names.len();
    if nmed > cfg.max_media {
        return Err(format!("{} media; $MXMED is {}", nmed, cfg.max_media));
    }
    let mel = cfg.max_elements;
    c.set_error_level(0);
    c.set_error_level_plus(0);
    let mut log = Log { emit: cfg.is_first(), lines: Vec::new() };
    let (ecut_min, pcut_min) = min_cuts(ecut, pcut);
    c.delimeter = fixed(b"MEDIA DEFINITION", 64);
    let mut u = Units { egsinp: InputUnit::from_text(egsinp), medfile: InputUnit::default(), delim_start: String::new(), delim_end: String::new() };
    let mut out = MediaInputs::default();

    // material data file (:743-767)
    let mut ival = 0usize;
    let mut medfile_specified = false;
    if let Some(name) = read_text(c, &mut u, false, &mut ival, "material data file")? {
        let name = cut(name, 256);
        match material_files.iter().find(|f| f.name == name) {
            Some(f) => {
                u.medfile = InputUnit::from_text(&f.text);
                medfile_specified = true;
                out.material_data_file = Some(name);
            }
            None => return Err(format!("Error: Cannot open material data file {}", name)),
        }
    } else {
        log.w(" Warning: material data file not supplied.");
        log.w(" Thus, you must define media explicitly in input file");
        log.w(" or via density correction file.");
    }

    // ae, ap, ue, up (:769-834)
    let mut limits = |c: &mut GetInputCommon, name: &str, dflt: f64| {
        ival += 1;
        c.declare(ival, name, InputType::Real, 1);
        c.set_limits(ival, 0.0, 999.0, dflt);
        ival
    };
    let iae = limits(c, "ae", ecut_min);
    let iap = limits(c, "ap", pcut_min);
    let iue = limits(c, "ue", 50.0 + cfg.prm);
    let iup = limits(c, "up", 50.0);
    c.nmin = iae;
    c.nmax = iup;
    u.call(false, c)?;
    let ae_tmp = if c.error_flags[iae] == 0 { c.value(iae, 1) } else { log.w(" Warning: AE for media not supplied.  Will use min. ECUT."); ecut_min };
    let ap_tmp = if c.error_flags[iap] == 0 { c.value(iap, 1) } else { log.w(" Warning: AP for media not supplied.  Will use min. PCUT."); pcut_min };
    let ue_tmp = if c.error_flags[iue] == 0 { c.value(iue, 1) } else { log.w(" Warning: UE for media not supplied.  Will use 50.5109989461 MeV"); 50.0 + cfg.prm };
    let up_tmp = if c.error_flags[iup] == 0 { c.value(iup, 1) } else { log.w(" Warning: UP for media not supplied.  Will use 50.0 MeV"); 50.0 };
    if ue_tmp <= ae_tmp {
        log.w(" Error: UE <= AE.  Adjust value(s) and try again.");
        return Err(String::from(" Error: UE <= AE.  Adjust value(s) and try again."));
    }
    if up_tmp <= ap_tmp {
        log.w(" Error: UP <= AP.  Adjust value(s) and try again.");
        return Err(String::from(" Error: UP <= AP.  Adjust value(s) and try again."));
    }
    (out.ae, out.ue, out.ap, out.up) = (ae_tmp, ue_tmp, ap_tmp, up_tmp);

    let mut t = Tmp {
        nne: 0,
        asym: vec![String::new(); mel],
        pz: vec![0.0; mel],
        rhoz: vec![0.0; mel],
        z: vec![0.0; mel],
        compound_list: vec![String::new(); mel],
        compound_fractions: vec![0.0; mel],
        rho: 0.0,
        localdensity: 0.0,
        ipot: 0.0,
        gasp: 0.0,
        sterncid: String::new(),
        iunrst: 0,
        iaprim: 1,
        epstfl: 0,
        density_file: String::new(),
        density_outputfile: String::new(),
        elements_specified: false,
        mixture_specified: false,
        rho_specified: false,
        spec_by_pz: false,
        spec_by_rhoz: false,
        stern_specified: false,
        iunrst_specified: false,
        iaprim_specified: false,
        gasp_specified: false,
        densityfile_specified: false,
    };

    for name_in in medium_names {
        let medium_name = cut(String::from(*name_in), 24);
        t.elements_specified = false;
        t.mixture_specified = false;
        t.rho_specified = false;
        t.densityfile_specified = false;
        t.stern_specified = false;
        t.iunrst_specified = false;
        t.iaprim_specified = false;
        t.gasp_specified = false;
        t.spec_by_rhoz = false;
        t.spec_by_pz = false;
        let (mut bulk, mut localspec, mut ipot_specified, mut outdens) = (false, false, false, false);
        let (mut mism_elem, mut mism_rho) = (false, false);
        t.sterncid = medium_name.clone();
        t.gasp = 0.0;
        t.iunrst = 0;
        t.iaprim = 1;
        t.epstfl = 0;
        t.density_file = String::new();
        log.w(" ");
        log.w(&format!(" For medium: {}", medium_name));
        c.delimeter = fixed(medium_name.as_bytes(), 64);
        let mut ival = 0usize;

        // composition in the deck (:864-936), then as a mixture (:938-985)
        read_elements(c, &mut u, false, &mut ival, &mut t, mel, &mut log)?;
        ival += 1;
        let imix = ival;
        c.declare(imix, "mixture compounds", InputType::Text, 0);
        c.seek(imix);
        u.call(false, c)?;
        if c.error_flags[imix] == 0 {
            if t.elements_specified {
                return Err(String::from(" Error: Both elements and mixture compounds have been specified. Please choose only one."));
            }
            ival += 1;
            let irz = ival;
            t.nne = c.nvalue[imix];
            if t.nne > mel || t.nne > MAX_VALUES {
                return Err(format!("{} compounds given; $MXEL is {}", t.nne, mel));
            }
            c.declare(irz, "mass fractions", InputType::Real, t.nne);
            c.seek(irz);
            if t.nne == 1 {
                c.set_limits(irz, 0.0, 1.0e15, 1.0);
            }
            u.call(false, c)?;
            if c.error_flags[irz] == 0 {
                for j in 1..=t.nne {
                    t.compound_list[j - 1] = cut(c.char_value(imix, j), 50);
                    t.compound_fractions[j - 1] = c.value(irz, j);
                }
                t.elements_specified = true;
                t.mixture_specified = true;
                t.spec_by_rhoz = true;
            }
            if t.elements_specified {
                log.w(" Composition specified in .egsinp file.");
            }
        }

        // densities (:987-1056)
        if let Some(v) = read_real(c, &mut u, false, &mut ival, "rho", 0.0, 1e15, 1.0)? {
            t.rho = v;
            t.rho_specified = true;
            log.w(" Rho specified in .egsinp file.");
        }
        if let Some(v) = read_real(c, &mut u, false, &mut ival, "bulk density", 0.0, 1e15, 1.0)? {
            t.rho = v;
            if t.rho_specified {
                let m = " Error: Both rho and bulk density specified. Either use only rho, or both bulk density and local density.If the bulk density and local density are equal, you can just provide the bulk density (or rho) and it will be used.";
                log.w(m);
                return Err(String::from(m));
            }
            t.rho_specified = true;
            bulk = true;
            log.w(" Bulk density specified in .egsinp file.");
        }
        if let Some(v) = read_real(c, &mut u, false, &mut ival, "local density", 0.0, 1e15, 1.0)? {
            t.localdensity = v;
            if !bulk {
                let m = " Error: Local density specified, but not bulk density. Please specify both bulk density and local density, or just the bulk density (or rho) if they are equal.";
                log.w(m);
                return Err(String::from(m));
            }
            localspec = true;
            log.w(" Local density specified in .egsinp file.");
        }
        if bulk && !localspec {
            log.w(" Warning: Bulk density specified, but not local density. Automatically setting local density equal to the bulk density.");
            localspec = true;
            t.localdensity = t.rho;
        }
        if let Some(f) = read_text(c, &mut u, false, &mut ival, "output density file")? {
            t.density_outputfile = cut(f, 256);
            outdens = true;
            log.w(" Density correction file will be output.");
        }
        // UPSTREAM? default -2 "so that we get an error if ivalue is negative"
        // (:1066): nothing checks for -2 afterwards; a negative ivalue just
        // becomes ipot = -2 here.
        if let Some(v) = read_real(c, &mut u, false, &mut ival, "ivalue", 0.0, 1e15, -2.0)? {
            t.ipot = v;
            ipot_specified = true;
            log.w(" I-value specified in .egsinp file.");
        }
        if let Some(s) = read_text(c, &mut u, false, &mut ival, "sterncid")? {
            t.sterncid = cut(s, 24);
            t.stern_specified = true;
            log.w(" STERNCID specified in .egsinp file.");
        }
        if let Some(v) = read_iunrst(c, &mut u, false, &mut ival)? {
            t.iunrst = v;
            t.iunrst_specified = true;
            log.w(" IUNRST specified in .egsinp file.");
        }
        if let Some(v) = read_iaprim(c, &mut u, false, &mut ival)? {
            t.iaprim = v;
            t.iaprim_specified = true;
            log.w(" IAPRIM specified in .egsinp file.");
        }
        if let Some(v) = read_real(c, &mut u, false, &mut ival, "gas pressure", 0.0, 1e15, 0.0)? {
            t.gasp = v;
            t.gasp_specified = true;
            log.w(" GASP specified in .egsinp file.");
        }
        if let Some(f) = read_text(c, &mut u, false, &mut ival, "density correction file")? {
            t.density_file = cut(f, 256);
            t.densityfile_specified = true;
            log.w(" Density correction file specified in .egsinp file.");
        }
        let spoutput = read_text(c, &mut u, false, &mut ival, "e- stopping power output file")?.map(|s| cut(s, 256));
        if let Some(f) = &spoutput {
            log.w(&format!(" e- stopping powers will be output to {}", f));
        }

        // the material data file (:1245-1527)
        if medfile_specified
            && (!t.elements_specified || !t.rho_specified || !t.iunrst_specified || !t.iaprim_specified || !t.gasp_specified || !t.stern_specified || !t.densityfile_specified)
        {
            let (found, ds, de) = find_medium_delimiters(&mut u.medfile, &medium_name);
            if !found {
                log.w(&format!(" Warning: Data for {} not found", medium_name));
                log.w(" in material data file.");
            } else {
                u.delim_start = ds;
                u.delim_end = de;
                let mut ival = 0usize;
                if !t.elements_specified {
                    read_elements(c, &mut u, true, &mut ival, &mut t, mel, &mut log)?;
                }
                if !t.rho_specified {
                    if let Some(v) = read_real(c, &mut u, true, &mut ival, "rho", 0.0, 1e15, 1.0)? {
                        t.rho = v;
                        t.rho_specified = true;
                        log.w(" Rho specified in material data file");
                    }
                }
                if !t.stern_specified {
                    if let Some(s) = read_text(c, &mut u, true, &mut ival, "sterncid")? {
                        t.sterncid = cut(s, 24);
                        t.stern_specified = true;
                        log.w(" STERNCID specified in material data file");
                    }
                }
                if !t.iunrst_specified {
                    if let Some(v) = read_iunrst(c, &mut u, true, &mut ival)? {
                        t.iunrst = v;
                        t.iunrst_specified = true;
                        log.w(" IUNRST specified in material data file");
                    }
                }
                if !t.iaprim_specified {
                    if let Some(v) = read_iaprim(c, &mut u, true, &mut ival)? {
                        t.iaprim = v;
                        t.iaprim_specified = true;
                        log.w(" IAPRIM specified in material data file");
                    }
                }
                if !t.gasp_specified {
                    if let Some(v) = read_real(c, &mut u, true, &mut ival, "gas pressure", 0.0, 1e15, 0.0)? {
                        t.gasp = v;
                        t.gasp_specified = true;
                        log.w(" GASP specified in material data file");
                    }
                }
                if !t.densityfile_specified {
                    if let Some(f) = read_text(c, &mut u, true, &mut ival, "density correction file")? {
                        t.density_file = cut(f, 256);
                        t.densityfile_specified = true;
                        log.w(" Density correction file specified in material data file.");
                    }
                }
            }
        }

        // the density correction file (:1529-1707)
        if t.densityfile_specified {
            let key = if t.density_file.contains('/') { t.density_file.clone() } else { format!("{}.density", t.density_file) };
            let Some(file) = density_files.iter().find(|f| f.name == key) else {
                log.w(&format!(" Error: Density correction file {} cannot be found.", key));
                return Err(format!("Error: Cannot open density correction file: {}", key));
            };
            t.density_file = key;
            t.epstfl = 1;
            let df = read_density_file(&file.text, mel)?;
            let mut asym_df: Vec<String> = Vec::new();
            for &z in &df.z {
                let i01 = z as i64;
                if !(1..=NET as i64).contains(&i01) {
                    return Err(format!("density correction file: Z = {} is not an element", z));
                }
                asym_df.push(String::from(ASYMT[i01 as usize - 1]));
            }
            if t.elements_specified {
                if t.nne != df.nne {
                    mism_elem = true;
                } else {
                    let mut rhoz_tot = 0.0;
                    for j in 0..t.nne {
                        if t.spec_by_pz {
                            let z = ztbl(&t.asym[j]).ok_or_else(|| format!("{} is not an atomic symbol", t.asym[j]))?;
                            t.rhoz[j] = t.pz[j] * watbl(z);
                        }
                        rhoz_tot += t.rhoz[j];
                    }
                    for j in 0..df.nne {
                        let mut found = false;
                        for k in 0..t.nne {
                            if asym_df[j] == t.asym[k] {
                                found = true;
                                if df.rhoz[j] > (1.0 + 0.01) * t.rhoz[k] / rhoz_tot || df.rhoz[j] < (1.0 - 0.01) * t.rhoz[k] / rhoz_tot {
                                    mism_elem = true;
                                }
                                break;
                            }
                        }
                        if !found {
                            mism_elem = true;
                        }
                        if mism_elem {
                            break;
                        }
                    }
                }
                if mism_elem {
                    log.w(" Warning: composition specified in density correction file is not the same as that");
                    log.w(" specified in input or material data file.");
                    log.w(" Will use the composition specified in the density correction file.");
                    t.nne = df.nne;
                    for j in 0..t.nne {
                        t.z[j] = df.z[j];
                        t.rhoz[j] = df.rhoz[j];
                        t.asym[j] = asym_df[j].clone();
                    }
                    t.spec_by_rhoz = true;
                }
            } else {
                log.w(" Composition specified in density correction file");
                t.nne = df.nne;
                for j in 0..t.nne {
                    t.z[j] = df.z[j];
                    t.rhoz[j] = df.rhoz[j];
                    t.asym[j] = asym_df[j].clone();
                }
                t.spec_by_rhoz = true;
                t.elements_specified = true;
            }
            if t.rho_specified {
                if df.rho > (1.0 + 0.01) * t.rho || df.rho < (1.0 - 0.01) * t.rho {
                    log.w(" Warning: rho specified in density correction file is not the same as that");
                    log.w(" specified in input or material data file.");
                    log.w(" Will use rho as specified in the density correction file.");
                    t.rho = df.rho;
                    mism_rho = true;
                }
            } else {
                t.rho = df.rho;
                t.rho_specified = true;
                log.w(" Rho specified in density correction file");
            }
            if t.gasp_specified {
                log.w(" Warning: gas pressure input not required when using density correction file.  Will set GASP=0.");
                t.gasp_specified = false;
                t.gasp = 0.0;
            }
        }

        // commit (:1709-1770)
        let mut m = MediumInput { name: medium_name.clone(), ..MediumInput::default() };
        if t.elements_specified && t.rho_specified {
            m.committed = true;
            (m.ae, m.ue, m.ap, m.up) = (ae_tmp, ue_tmp, ap_tmp, up_tmp);
            m.sterncid = t.sterncid.clone();
            m.rho = t.rho;
            m.local_density = if localspec { t.localdensity as f32 } else { t.rho as f32 };
            m.ipot = if ipot_specified { t.ipot as f32 } else { -1.0 };
            m.iscompound = 0;
            if !t.mixture_specified {
                m.nne = t.nne;
                for j in 0..t.nne {
                    let z = ztbl(&t.asym[j]).ok_or_else(|| format!("{} is not an atomic symbol", t.asym[j]))?;
                    let wa = watbl(z);
                    let wa4 = WATBL[z - 1];
                    let (mut pz, mut rhoz, mut pz4, mut rhoz4) = (0.0, 0.0, 0.0f32, 0.0f32);
                    if t.spec_by_rhoz {
                        rhoz = t.rhoz[j];
                        rhoz4 = rhoz as f32;
                        pz = rhoz / wa;
                        pz4 = rhoz4 / wa4;
                    } else if t.spec_by_pz {
                        pz = t.pz[j];
                        pz4 = pz as f32;
                        rhoz = pz * wa;
                        rhoz4 = pz4 * wa4;
                    }
                    m.elements.push(MediumElement { element: Pegs4Element { asym: t.asym[j].clone(), zelem: z as f64, wa, pz, rhoz }, pz4, rhoz4, wa4 });
                }
                if t.spec_by_pz && m.nne > 1 {
                    m.iscompound = 1;
                }
            } else {
                m.ncomp = t.nne;
                for j in 0..t.nne {
                    m.compound_list.push(t.compound_list[j].clone());
                    m.compound_fractions.push(t.compound_fractions[j] as f32);
                }
                // egs_compoundstoelements (:1720) -> estar_cpp
                let formulas = super::estar_cpp::element_array(&t.compound_list[..t.nne].join(" "), t.nne).map_err(|e| format!("{e:?}"))?;
                let fracs: Vec<f64> = t.compound_fractions[..t.nne].to_vec();
                let expanded = super::estar_cpp::compounds_to_elements(&formulas, &fracs, m.local_density).map_err(|e| format!("{e:?}"))?;
                m.nne = expanded.len();
                for e in &expanded {
                    let z = e.z as usize;
                    let (wa, wa4) = (watbl(z), WATBL[z - 1]);
                    let rhoz = e.rhoz;
                    let rhoz4 = rhoz as f32;
                    let asym = String::from(super::estar_cpp::symbol(e.z).unwrap_or(""));
                    m.elements.push(MediumElement { element: Pegs4Element { asym, zelem: f64::from(e.z), wa, pz: rhoz / wa, rhoz }, pz4: rhoz4 / wa4, rhoz4, wa4 });
                }
            }
            m.iunrst = t.iunrst;
            m.iaprim = t.iaprim;
            m.epstfl = t.epstfl;
            m.inpgasp = t.gasp as f32;
            m.inpdensity_file = t.density_file.clone();
            if outdens && cfg.is_first() {
                m.outdensity_file = Some(t.density_outputfile.clone());
            }
        } else {
            log.w(&format!(" Error: Medium {} not correctly defined.", medium_name));
        }
        m.spoutput_file = spoutput;
        m.df_if_elem_mismatch = mism_elem;
        m.df_if_rho_mismatch = mism_rho;
        out.media.push(m);
    }
    out.mederr = log.lines;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFS: &str = ":start media definition:\n ae = 0.521\n ap = 0.01\n ue = 55.0\n up = 50.0\n:stop media definition:\n";

    fn cfg() -> MediaInputConfig {
        MediaInputConfig::new(10, 10)
    }

    fn run_with(deck: &str, names: &[&str], mat: &[MediaFile], dens: &[MediaFile]) -> Result<MediaInputs, String> {
        let mut c = GetInputCommon::new();
        get_media_inputs(deck, names, &[0.7, 0.5], &[0.1], mat, dens, &cfg(), &mut c)
    }

    fn run(deck: &str, names: &[&str]) -> Result<MediaInputs, String> {
        run_with(deck, names, &[], &[])
    }

    fn file(name: &str, text: &str) -> MediaFile {
        MediaFile { name: String::from(name), text: String::from(text) }
    }

    #[test]
    fn get_media_inputs_tables_ztbl_and_min_cuts() {
        assert_eq!(ztbl("H"), Some(1));
        assert_eq!(ztbl("HE"), Some(2));
        assert_eq!(ztbl("FM"), Some(100));
        assert_eq!(ztbl("A"), Some(18));
        assert_eq!(ztbl("AR"), Some(18));
        assert_eq!(ztbl("XX"), None);
        assert_eq!(ztbl("h"), None);
        assert_eq!(ztbl("CARBON"), None);
        assert_eq!(watbl(1), f64::from(1.00797_f32));
        assert_eq!(WATBL[99], 253.0);
        assert_eq!(min_cuts(&[0.7, 0.5], &[0.1]), (0.5, 0.1));
        assert_eq!(min_cuts(&[], &[]), (999.0, 999.0));
        assert!(MediaInputConfig::new(1, 1).is_first());
        let mut p = MediaInputConfig::new(1, 1);
        p.n_parallel = 2;
        p.i_parallel = 2;
        p.first_parallel = 1;
        assert!(!p.is_first());
    }

    #[test]
    fn get_media_inputs_deck_composition_two_media() {
        let deck = format!(
            "{DEFS}:start H2O:\n  elements = h, o\n  number of atoms = 2, 1\n  rho = 1.0\n:stop H2O:\n\
             :start AIR:\n  elements = N, O\n  mass fractions = 0.78, 0.22\n  rho = 0.0012\n  gas pressure = 2.0\n:stop AIR:\n"
        );
        let r = run(&deck, &["H2O", "AIR"]).unwrap();
        assert_eq!((r.ae, r.ue, r.ap, r.up), (0.521, 55.0, 0.01, 50.0));
        assert!(r.mederr.iter().any(|l| l.contains("material data file not supplied")));
        let w = &r.media[0];
        assert!(w.committed);
        assert_eq!((w.nne, w.iscompound, w.rho), (2, 1, 1.0));
        assert_eq!(w.elements[0].element.asym, "H");
        assert_eq!(w.elements[0].element.zelem, 1.0);
        assert_eq!(w.elements[0].element.pz, 2.0);
        assert_eq!(w.elements[0].element.rhoz, 2.0 * f64::from(1.00797_f32));
        assert_eq!(w.elements[0].wa4, 1.00797_f32);
        assert_eq!(w.elements[1].element.rhoz, f64::from(15.9994_f32));
        assert_eq!((w.local_density, w.ipot, w.sterncid.as_str()), (1.0, -1.0, "H2O"));
        assert_eq!((w.iunrst, w.iaprim, w.epstfl, w.inpgasp), (0, 1, 0, 0.0));
        assert_eq!((w.ae, w.ue, w.ap, w.up), (0.521, 55.0, 0.01, 50.0));
        let a = &r.media[1];
        assert_eq!(a.iscompound, 0);
        assert_eq!(a.elements[0].element.rhoz, 0.78);
        assert_eq!(a.elements[0].element.pz, 0.78 / f64::from(14.0067_f32));
        assert_eq!(a.elements[0].pz4, 0.78_f32 / 14.0067_f32);
        assert_eq!(a.inpgasp, 2.0);
        assert!(a.inpdensity_file.is_empty());
    }

    #[test]
    fn get_media_inputs_defaults_when_no_media_definition_block() {
        let deck = ":start X:\n elements = C\n rho = 2.0\n:stop X:\n";
        let r = run(deck, &["X"]).unwrap();
        assert_eq!((r.ae, r.ap), (0.5, 0.1));
        assert_eq!(r.ue, 50.0 + RM);
        assert_eq!(r.up, 50.0);
        // a single element: pz 1, default atomic weight
        let m = &r.media[0];
        assert_eq!((m.nne, m.elements[0].element.pz, m.iscompound), (1, 1.0, 0));
        assert_eq!(m.elements[0].element.rhoz, f64::from(12.01115_f32));
        for l in ["AE for media not supplied", "AP for media", "UE for media", "UP for media"] {
            assert!(r.mederr.iter().any(|m| m.contains(l)), "{l}");
        }
        // an out of range value falls back to the default (999 is the maximum)
        let deck = ":start media definition:\n ae = 1000.0\n:stop media definition:\n:start X:\n elements = C\n rho = 2.0\n:stop X:\n";
        assert_eq!(run(deck, &["X"]).unwrap().ae, 0.5);
    }

    #[test]
    fn get_media_inputs_single_element_by_mass_fraction_and_options() {
        let deck = format!(
            "{DEFS}:start PB:\n elements = PB\n mass fractions = 1.0\n bulk density = 11.35\n local density = 11.0\n ivalue = 0.000823\n \
             sterncid = LEAD\n stopping powers = unrestricted radiative\n bremsstrahlung correction = none\n \
             output density file = out\n e- stopping power output file = pb.xvg\n:stop PB:\n"
        );
        let m = &run(&deck, &["PB"]).unwrap().media[0];
        assert!(m.committed);
        assert_eq!((m.rho, m.local_density, m.ipot), (11.35, 11.0, 0.000823_f32));
        assert_eq!((m.sterncid.as_str(), m.iunrst, m.iaprim), ("LEAD", 5, 2));
        assert_eq!(m.elements[0].element.rhoz, 1.0);
        assert_eq!(m.outdensity_file.as_deref(), Some("out"));
        assert_eq!(m.spoutput_file.as_deref(), Some("pb.xvg"));
        // not the first parallel job: no output density file, no .mederr text
        let mut p = cfg();
        p.n_parallel = 2;
        p.i_parallel = 2;
        p.first_parallel = 1;
        let mut c = GetInputCommon::new();
        let r = get_media_inputs(&deck, &["PB"], &[0.7], &[0.1], &[], &[], &p, &mut c).unwrap();
        assert!(r.media[0].outdensity_file.is_none());
        assert!(r.mederr.is_empty());
    }

    #[test]
    fn get_media_inputs_density_rules_and_errors() {
        let blk = |body: &str| format!("{DEFS}:start M:\n elements = C\n{body}:stop M:\n");
        // bulk density alone: local density follows
        let r = run(&blk(" bulk density = 3.0\n"), &["M"]).unwrap();
        assert_eq!((r.media[0].rho, r.media[0].local_density), (3.0, 3.0));
        assert!(r.mederr.iter().any(|l| l.contains("Automatically setting local density")));
        // rho together with bulk density
        let e = run(&blk(" rho = 3.0\n bulk density = 3.0\n"), &["M"]).unwrap_err();
        assert!(e.contains("Both rho and bulk density"));
        // local density without bulk density
        let e = run(&blk(" rho = 3.0\n local density = 2.0\n"), &["M"]).unwrap_err();
        assert!(e.contains("Local density specified, but not bulk"));
        // elements together with mixture compounds
        let e = run(&blk(" mixture compounds = A, B\n mass fractions = 0.5, 0.5\n rho = 1.0\n"), &["M"]).unwrap_err();
        assert!(e.contains("Both elements and mixture"));
        // no rho: the medium is not correctly defined, not fatal
        let r = run(&blk(""), &["M"]).unwrap();
        assert!(!r.media[0].committed);
        assert!(r.mederr.iter().any(|l| l.contains("not correctly defined")));
        // an unknown symbol is reported (Fortran would index WATBL(0))
        let e = run(&format!("{DEFS}:start M:\n elements = XX\n rho = 1.0\n:stop M:\n"), &["M"]).unwrap_err();
        assert!(e.contains("not an atomic symbol"));
    }

    #[test]
    fn get_media_inputs_mixture_is_expanded_to_elements() {
        let deck = format!("{DEFS}:start MIX:\n mixture compounds = H2O, C1H4\n mass fractions = 0.25, 0.75\n rho = 1.5\n:stop MIX:\n");
        let m = &run(&deck, &["MIX"]).unwrap().media[0];
        assert!(m.committed && !m.needs_compound_expansion);
        assert_eq!((m.ncomp, m.nne, m.elements.len()), (2, 3, 3));
        assert_eq!(m.elements.iter().map(|e| e.element.asym.as_str()).collect::<Vec<_>>(), vec!["H", "C", "O"]);
        let total: f64 = m.elements.iter().map(|e| e.element.rhoz).sum();
        assert!((total - 1.0).abs() < 1e-14);
        assert_eq!(m.compound_list, vec![String::from("H2O"), String::from("C1H4")]);
        assert_eq!(m.compound_fractions, vec![0.25_f32, 0.75_f32]);
        assert_eq!(m.rho, 1.5);
    }

    #[test]
    fn get_media_inputs_threshold_errors() {
        let e = run(":start media definition:\n ae = 0.6\n ue = 0.5\n:stop media definition:\n", &[]).unwrap_err();
        assert!(e.contains("UE <= AE"));
        let e = run(":start media definition:\n ap = 0.6\n up = 0.5\n ue = 1.5\n:stop media definition:\n", &[]).unwrap_err();
        assert!(e.contains("UP <= AP"));
        // more media than $MXMED
        let names = ["A"; 11];
        assert!(run("", &names).unwrap_err().contains("$MXMED"));
        // more elements than $MXEL
        let list: Vec<&str> = vec!["H"; 11];
        let deck = format!("{DEFS}:start M:\n elements = {}\n:stop M:\n", list.join(", "));
        assert!(run(&deck, &["M"]).unwrap_err().contains("$MXEL"));
    }

    const MAT: &str = "medium = WATER\n  elements = H, O\n  number of atoms = 2, 1\n  rho = 1.0\n  stopping powers = unrestricted collision\n  \
                       bremsstrahlung correction = KM\n  gas pressure = 1.5\nmedium = LEAD\n  elements = PB\n  rho = 11.35\n";

    #[test]
    fn get_media_inputs_material_data_file_entries() {
        let deck = format!(":start media definition:\n material data file = mat.dat\n:stop media definition:\n:start WATER:\n:stop WATER:\n:start LEAD:\n sterncid = PBX\n:stop LEAD:\n:start GOLD:\n:stop GOLD:\n");
        let r = run_with(&deck, &["WATER", "LEAD", "GOLD"], &[file("mat.dat", MAT)], &[]).unwrap();
        assert_eq!(r.material_data_file.as_deref(), Some("mat.dat"));
        let w = &r.media[0];
        assert!(w.committed);
        assert_eq!((w.nne, w.rho, w.iunrst, w.iaprim, w.inpgasp), (2, 1.0, 1, 0, 1.5));
        assert_eq!(w.iscompound, 1);
        let l = &r.media[1];
        assert_eq!((l.nne, l.rho, l.sterncid.as_str()), (1, 11.35, "PBX"));
        assert_eq!(l.elements[0].element.zelem, 82.0);
        // GOLD is not in the file
        assert!(!r.media[2].committed);
        assert!(r.mederr.iter().any(|m| m.contains("Data for GOLD not found")));
        // a missing material data file is fatal
        let e = run_with(&deck, &["WATER"], &[], &[]).unwrap_err();
        assert!(e.contains("Cannot open material data file"));
        // lower-case symbols from a material data file are not upper-cased (UPSTREAM?)
        let lower = "medium = W\n elements = pb\n rho = 1.0\n";
        let d = ":start media definition:\n material data file = m\n:stop media definition:\n:start W:\n:stop W:\n";
        assert!(run_with(d, &["W"], &[file("m", lower)], &[]).unwrap_err().contains("not an atomic symbol"));
    }

    const DENS: &str = "AIR density effect\n 0 85.7 0.0012 2\n 7 0.78  8 0.22\n";

    #[test]
    fn get_media_inputs_density_correction_file() {
        // composition and rho taken from the file
        let deck = format!("{DEFS}:start AIR:\n density correction file = air\n gas pressure = 3.0\n:stop AIR:\n");
        let r = run_with(&deck, &["AIR"], &[], &[file("air.density", DENS)]).unwrap();
        let m = &r.media[0];
        assert!(m.committed);
        assert_eq!((m.epstfl, m.nne, m.rho, m.inpdensity_file.as_str()), (1, 2, 0.0012, "air.density"));
        assert_eq!(m.elements[0].element.asym, "N");
        assert_eq!(m.elements[1].element.rhoz, 0.22);
        // gas pressure is discarded with a warning
        assert_eq!(m.inpgasp, 0.0);
        assert!(r.mederr.iter().any(|l| l.contains("gas pressure input not required")));
        // matching composition by atoms is kept (no mismatch), rho within 1 %
        let ok = format!(
            "{DEFS}:start AIR:\n elements = N, O\n mass fractions = 0.78, 0.22\n rho = 0.00121\n density correction file = air\n:stop AIR:\n"
        );
        let m = &run_with(&ok, &["AIR"], &[], &[file("air.density", DENS)]).unwrap().media[0];
        assert!(!m.df_if_elem_mismatch && !m.df_if_rho_mismatch);
        assert_eq!(m.rho, 0.00121);
        // different composition and rho: the file wins, both mismatches flagged
        let bad = format!(
            "{DEFS}:start AIR:\n elements = N, O\n number of atoms = 1, 1\n rho = 0.5\n density correction file = air\n:stop AIR:\n"
        );
        let m = &run_with(&bad, &["AIR"], &[], &[file("air.density", DENS)]).unwrap().media[0];
        assert!(m.df_if_elem_mismatch && m.df_if_rho_mismatch);
        assert_eq!((m.rho, m.elements[0].element.rhoz), (0.0012, 0.78));
        // a different element count
        let one = format!("{DEFS}:start AIR:\n elements = N\n rho = 0.0012\n density correction file = air\n:stop AIR:\n");
        assert!(run_with(&one, &["AIR"], &[], &[file("air.density", DENS)]).unwrap().media[0].df_if_elem_mismatch);
        // a path with a separator is used as given
        let path = format!("{DEFS}:start AIR:\n density correction file = /d/air.density\n:stop AIR:\n");
        assert!(run_with(&path, &["AIR"], &[], &[file("/d/air.density", DENS)]).unwrap().media[0].committed);
        // missing file is fatal
        let e = run_with(&deck, &["AIR"], &[], &[]).unwrap_err();
        assert!(e.contains("Cannot open density correction file"));
        // bad contents
        for bad in ["", "t\n 0 1 0.1 1\n 200 1\n", "t\n 0 1 0.1 99\n", "t\n 0 1 x 1\n"] {
            assert!(run_with(&deck, &["AIR"], &[], &[file("air.density", bad)]).is_err(), "{bad:?}");
        }
        // mass fractions given as atoms are normalised through WATBL for the comparison
        let atoms = format!(
            "{DEFS}:start AIR:\n elements = N, O\n number of atoms = 1, 1\n rho = 0.0012\n density correction file = air\n:stop AIR:\n"
        );
        let d2 = "t\n 0 1 0.0012 2\n 7 0.46681 8 0.53319\n";
        let m = &run_with(&atoms, &["AIR"], &[], &[file("air.density", d2)]).unwrap().media[0];
        assert!(!m.df_if_elem_mismatch);
    }

    #[test]
    fn get_media_inputs_header_scan_and_density_reader() {
        let mut u = InputUnit::from_text("x\nMedium = A\nrho=1\n medium=B\nmedium = A\n");
        let (f, s, e) = find_medium_delimiters(&mut u, "A");
        assert!(f);
        assert_eq!((s.as_str(), e.as_str()), ("Medium = A", " medium=B"));
        let (f, _, e) = find_medium_delimiters(&mut u, "B");
        assert!(f);
        assert_eq!(e, "medium = A");
        let (f, _, _) = find_medium_delimiters(&mut u, "Z");
        assert!(!f);
        let d = read_density_file(DENS, 5).unwrap();
        assert_eq!((d.nne, d.rho, d.z.clone(), d.rhoz.clone()), (2, 0.0012, vec![7.0, 8.0], vec![0.78, 0.22]));
        assert_eq!(cut(String::from("abcdef  "), 3), "abc");
    }
}
