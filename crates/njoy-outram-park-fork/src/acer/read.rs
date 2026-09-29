//! Read an existing ACE file — upstream `acer`'s `iopt = 7` / `iopt = 8`.
//!
//! # Why this module exists
//!
//! Until 2026-09-21 this crate could **write** ACE tables and could not read
//! one. Every comparison against NJOY2016 therefore carried its own ad-hoc
//! Type-1 parser: `examples/ace_vs_njoy2016.rs`, `examples/thermal_ace_vs_njoy2016.rs`,
//! `tests/acer.rs`, `tests/thermal_ace.rs` and `tests/thermal_ace_zrh.rs` had
//! **five** copies of the same 30 lines. That is exactly the duplication the
//! workspace rule against re-building what exists is meant to prevent, and two
//! of those copies had already drifted.
//!
//! # What upstream reads, and what this reads
//!
//! `acer.f90:485-533` dispatches `iopt = 7` (Type 1) and `iopt = 8` (Type 2,
//! binary) — note `itype = iopt - 6` — on the **class letter** of the ZAID:
//!
//! | letter | class | upstream routine |
//! |---|---|---|
//! | `c` | continuous-energy neutron | `acefix` |
//! | `h` `o` `r` `s` `a` | charged particle | `acefix` |
//! | `t` | thermal S(alpha,beta) | `thrfix` |
//! | `p` | photoatomic | `phofix` |
//! | `u` | photonuclear | `phnfix` |
//! | `y` | dosimetry | `dosfix` |
//!
//! This module reads the **container** — header, `NXS`, `JXS`, `XSS` — for
//! every one of those classes, because the container is identical across them;
//! only the meaning of the blocks differs, and that is the caller's business.
//! It does not *interpret* a dosimetry or photonuclear table, and says so
//! rather than pretending to.
//!
//! # The ZAID is not always a number
//!
//! Upstream reads the ZAID as `a9` first and only re-reads it as `f9.0` **when
//! the class is not `t`** (`acer.f90:505-508`), because a thermal ZAID such as
//! `al27.00t` has no numeric form. [`AceHeader::zaid_num`] is therefore
//! `Option<f64>`, not `f64`. Getting this wrong is silent: a parser that
//! unconditionally parses the ZAID as a float either panics on every thermal
//! table or, worse, silently accepts a partial parse.

use std::path::Path;

use crate::acer::fortran_fmt::{
    fixed_field, fortran_e, fortran_e20, fortran_f, fortran_f0, fortran_i20,
};
use crate::NjoyError;

/// Which ACE class a table belongs to, from the ZAID's trailing letter.
///
/// The letter is the only thing in the file that says what the blocks mean, so
/// it is carried explicitly rather than inferred from block contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AceClass {
    /// `c` — continuous-energy neutron.
    ContinuousNeutron,
    /// `h`, `o`, `r`, `s`, `a` — incident charged particle. The letter is kept
    /// because upstream routes all five to `acefix` but they are not the same
    /// projectile (`a` is the incident alpha this workspace holds for He-4).
    ChargedParticle(char),
    /// `t` — thermal S(alpha,beta).
    Thermal,
    /// `p` — photoatomic.
    Photoatomic,
    /// `u` — photonuclear.
    Photonuclear,
    /// `y` — dosimetry.
    Dosimetry,
}

impl AceClass {
    /// Classify from the ZAID's trailing letter, as `acer.f90:513-533` does.
    pub fn from_letter(c: char) -> Option<AceClass> {
        match c {
            'c' => Some(AceClass::ContinuousNeutron),
            'h' | 'o' | 'r' | 's' | 'a' => Some(AceClass::ChargedParticle(c)),
            't' => Some(AceClass::Thermal),
            'p' => Some(AceClass::Photoatomic),
            'u' => Some(AceClass::Photonuclear),
            'y' => Some(AceClass::Dosimetry),
            _ => None,
        }
    }

    /// The letter this class is written with (`ChargedParticle` returns its own).
    pub fn letter(self) -> char {
        match self {
            AceClass::ContinuousNeutron => 'c',
            AceClass::ChargedParticle(c) => c,
            AceClass::Thermal => 't',
            AceClass::Photoatomic => 'p',
            AceClass::Photonuclear => 'u',
            AceClass::Dosimetry => 'y',
        }
    }
}

/// The two ACE container formats upstream reads (`iopt = 7` and `iopt = 8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AceFileType {
    /// Type 1 — formatted ASCII.
    Type1Ascii,
    /// Type 2 — Fortran unformatted sequential (`acefc.f90:13028-13053`).
    Type2Binary,
}

/// An ACE table's header block, common to every class.
#[derive(Debug, Clone)]
pub struct AceHeader {
    /// `hz` exactly as stored, e.g. `"92235.00c"` or `"al27.00t"`.
    pub zaid: String,
    /// The numeric ZA, **`None` for a thermal table** whose ZAID is a name.
    pub zaid_num: Option<f64>,
    /// The class letter's meaning.
    pub class: AceClass,
    /// `aw0` — atomic weight ratio.
    pub awr: f64,
    /// `tz` — temperature as `kT` \[MeV\].
    pub kt_mev: f64,
    /// `hd` — processing date.
    pub date: String,
    /// `hk` — the 70-character free-text comment.
    pub comment: String,
    /// `hm` — the material id string, e.g. `"   mat9228"`.
    pub mat_id: String,
    /// The 16 `IZ` entries of the IZ/AW pair block.
    pub iz: [i32; 16],
    /// The 16 `AW` entries of the IZ/AW pair block.
    pub aw: [f64; 16],
    /// The four Fortran `character(n)` fields **exactly as stored**, in order:
    /// `hz`, `hd`, `hk`, `hm`.
    ///
    /// The trimmed fields above are what a caller wants to read; these are what
    /// a byte-exact rewrite needs. NJOY does not always space-pad: Al-27's
    /// Type-2 `hz` is `13027.00c` followed by byte `0x0E`, so reconstructing
    /// the field from the trimmed string and a padding rule cannot reproduce
    /// it. Writing these back verbatim is the only way a read-then-write
    /// round trip is byte-exact, and that round trip is the whole point of
    /// upstream's `iopt = 7/8`.
    pub raw_text: [Vec<u8>; 4],
}

/// A complete ACE table as stored: header plus the three raw arrays.
///
/// Deliberately *raw*. Interpreting `xss` needs the class-specific `jxs`
/// locators, and this type does not pretend to know which class it is beyond
/// recording [`AceHeader::class`].
#[derive(Debug, Clone)]
pub struct RawAceTable {
    /// Which container this came from.
    pub file_type: AceFileType,
    /// The header block.
    pub header: AceHeader,
    /// `NXS(1..16)`, 0-based.
    pub nxs: [i32; 16],
    /// `JXS(1..32)`, 0-based. Locators are **1-based into `xss`** as stored.
    pub jxs: [i32; 32],
    /// The `XSS` data block.
    pub xss: Vec<f64>,
    /// Which `xss` words the Type-1 writer must print as integers (`typen`'s
    /// `iflag = 1`, `acecm.f90:790`), when that is known.
    ///
    /// It is known for a table this crate **built** — each class's writer says
    /// word by word which are integers — and unknowable for a table **read**
    /// from a file, because the file does not record it. `None` means "infer
    /// it", which [`to_type1_string`][Self::to_type1_string] does the only way
    /// available: an exactly-integral value small enough to be a locator,
    /// count or MT number is written as one. That reproduces NJOY on the
    /// tables tested and is a guess in general, which is why a builder should
    /// supply the mask instead of relying on it.
    pub xss_is_int: Option<Vec<bool>>,
}

impl RawAceTable {
    /// `xss` at a **1-based** `JXS` locator, which is how every locator in the
    /// file is expressed. Returns `None` for locator `0` ("block absent") and
    /// for an out-of-range index, so a malformed table cannot panic here.
    pub fn at(&self, locator: i32) -> Option<f64> {
        if locator <= 0 {
            return None;
        }
        self.xss.get((locator - 1) as usize).copied()
    }

    /// `n` values starting at a **1-based** locator. `None` if the span does
    /// not fit, rather than a truncated slice.
    pub fn slice_at(&self, locator: i32, n: usize) -> Option<&[f64]> {
        if locator <= 0 {
            return None;
        }
        let lo = (locator - 1) as usize;
        self.xss.get(lo..lo + n)
    }
}

/// Split the ZAID string into its numeric part (when there is one) and class.
///
/// Mirrors `acer.f90:505-511`: read the first characters as text, take the
/// class letter, and only attempt a numeric ZA when the class is not `t`.
fn split_zaid(hz: &str) -> Result<(Option<f64>, AceClass), NjoyError> {
    let trimmed = hz.trim();
    let letter = trimmed.chars().last().ok_or_else(|| {
        NjoyError::EndfParse("ACE header: empty ZAID field".into())
    })?;
    let class = AceClass::from_letter(letter).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "ACE header: ZAID {trimmed:?} ends in {letter:?}, which is not one of \
             upstream's class letters c/h/o/r/s/a/t/p/u/y (acer.f90:513-533)"
        ))
    })?;
    // Strip **every** trailing letter, not just one. The mcnpx variant writes a
    // two-character class string (`"pp"`, `"ny"`, `"nt"`, `"nc"`), so taking a
    // single character off `6000.000pp` leaves `6000.000p`, which does not
    // parse and would silently lose the ZA. The class is still the last letter,
    // which is the same in both conventions — `'p'`, `'y'`, `'t'`, `'c'`.
    let body = trimmed.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let zaid_num = if class == AceClass::Thermal {
        // A thermal ZAID is a name ("al27.00"), not a number — upstream skips
        // the numeric read for exactly this case.
        None
    } else {
        body.trim().parse::<f64>().ok()
    };
    Ok((zaid_num, class))
}

/// Read a **Type 1** (ASCII) ACE file.
///
/// Layout, per `acefc.f90`'s type-1 writer and `change`:
///
/// ```text
/// line 1      zaid(10)  awr  tz  date
/// line 2      hk(70) + hm(10)
/// lines 3-6   16 IZ/AW pairs, 4 pairs per line
/// lines 7-8   NXS(16), 8 integers per line
/// lines 9-12  JXS(32), 8 integers per line
/// lines 13-   XSS, 4 values per line
/// ```
///
/// The header's fixed-width fields are taken **by column** where the format is
/// fixed (the 70-character comment can contain spaces, so splitting it on
/// whitespace corrupts it); the numeric blocks are whitespace-split, which is
/// safe because NJOY writes them space-separated.
pub fn read_type1<P: AsRef<Path>>(path: P) -> Result<RawAceTable, NjoyError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| NjoyError::EndfParse(format!("read {}: {e}", path.display())))?;
    parse_type1(&text)
}

/// Parse a Type-1 ACE table already in memory. Split out from [`read_type1`] so
/// it is testable without a file.
///
/// A file holding **several** tables is refused here, by name, rather than
/// silently read as its first one: use [`parse_type1_library`] or
/// [`read_table`]. (Before 2026-09-29 such a file failed with an XSS-length
/// error that did not say why.)
pub fn parse_type1(text: &str) -> Result<RawAceTable, NjoyError> {
    let mut tables = parse_type1_library(text)?;
    match tables.len() {
        1 => Ok(tables.pop().expect("length checked")),
        0 => Err(NjoyError::EndfParse("ACE Type 1: no table in the text".into())),
        n => Err(NjoyError::EndfParse(format!(
            "ACE Type 1: the file holds {n} tables ({}); read one by name with \
             `acer::read::read_table`, or all with `read_library`",
            tables.iter().map(|t| t.header.zaid.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// Parse **every** Type-1 table in `text`, in file order — an MCNP/OpenMC-style
/// library file concatenates them. GitHub #365 audit.
///
/// A port of OpenMC's `Library._read_ascii` (`openmc/data/ace.py:326-420`):
///
/// - Tables follow one another with no separator. A table's XSS is exactly
///   `NXS(1)` values, so the next table starts on the line after its last one
///   (counted in values here, not lines, so a writer's values-per-line is not
///   assumed).
/// - **The ACE 2.0 header** is recognised as OpenMC recognises it: the first
///   token's second character is `.` (`2.0.1`). Its first line is
///   `version name source`, its second `awr kT date n`, followed by `n` comment
///   lines, and then the legacy IZ/AW, NXS, JXS and XSS layout. The comment
///   lines are kept, joined, as `header.comment`; `mat_id` is empty because the
///   2.0 header has no such field.
/// - **NJOY's exponent-less floats** — `1.0-120`, written for values below
///   1e-100 — are read as `1.0e-120`, as OpenMC's `ENDF_FLOAT_RE` fallback does.
pub fn parse_type1_library(text: &str) -> Result<Vec<RawAceTable>, NjoyError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut at = 0usize;
    loop {
        while at < lines.len() && lines[at].trim().is_empty() {
            at += 1;
        }
        if at >= lines.len() {
            break;
        }
        let (table, next) = parse_type1_at(&lines, at)?;
        out.push(table);
        at = next;
    }
    Ok(out)
}

/// A float as NJOY's ACER writes it, including the exponent-less form
/// `1.0-120` (a Fortran `e` edit descriptor drops the `e` for three-digit
/// exponents). The fallback runs only when the ordinary parse fails.
fn parse_ace_float(t: &str) -> Option<f64> {
    if let Ok(v) = t.parse::<f64>() {
        return Some(v);
    }
    let b = t.as_bytes();
    // The exponent sign: the last '+' or '-' that is not the leading sign and
    // does not follow an 'e'/'E'.
    let k = (1..b.len()).rev().find(|&k| {
        (b[k] == b'+' || b[k] == b'-') && !matches!(b[k - 1], b'e' | b'E')
    })?;
    format!("{}e{}", &t[..k], &t[k..]).parse().ok()
}

/// Parse one Type-1 table whose first line is `lines[start]`, returning it and
/// the index of the first line after it.
fn parse_type1_at(lines: &[&str], start: usize) -> Result<(RawAceTable, usize), NjoyError> {
    let lines = &lines[start..];
    if lines.len() < 12 {
        return Err(NjoyError::EndfParse(format!(
            "ACE Type 1: {} lines, but the header alone is 12",
            lines.len()
        )));
    }
    // ACE 2.0 header (`openmc/data/ace.py:349-359`): `2.0.1 name source`.
    let is_v2 = lines[0]
        .split_whitespace()
        .next()
        .is_some_and(|w| w.as_bytes().get(1) == Some(&b'.'));
    if is_v2 {
        return parse_type1_v2(lines).map(|(t, n)| (t, start + n));
    }
    // Line 1: the ZAID is a fixed field, 10 columns normally and 13 in the
    // mcnpx variant (`acer.f90:490-494` picks between them from a user flag).
    // A *file* carries no such flag, so the width is decided from the bytes:
    // columns 11-13 are the mcnpx class suffix (`'pp '`, `'ny '`, `'nt '`,
    // `'nc '`) in that variant and the first three columns of the `f12.6` AWR
    // otherwise -- and an `f12.6` field cannot contain a letter.
    let l0 = lines[0];
    let hz_len = if l0
        .as_bytes()
        .get(10..13)
        .is_some_and(|b| b.iter().any(|c| c.is_ascii_alphabetic()))
    {
        13
    } else {
        10
    };
    let zaid_raw: Vec<u8> = l0.bytes().take(hz_len).collect();
    let zaid = String::from_utf8_lossy(&zaid_raw).trim().to_string();
    let rest: Vec<&str> = l0.get(hz_len..).unwrap_or("").split_whitespace().collect();
    if rest.len() < 2 {
        return Err(NjoyError::EndfParse(format!(
            "ACE Type 1 line 1: expected awr and kT after the ZAID, got {l0:?}"
        )));
    }
    let awr = rest[0]
        .parse()
        .map_err(|e| NjoyError::EndfParse(format!("ACE awr {:?}: {e}", rest[0])))?;
    let kt_mev = rest[1]
        .parse()
        .map_err(|e| NjoyError::EndfParse(format!("ACE kT {:?}: {e}", rest[1])))?;
    // The date is a fixed `a10` field, not a free token: the line is
    // `a10/a13, f12.6, 1x, 1pe11.4, 1x, a10`, so it starts 25 columns past the
    // ZAID. Slicing it keeps NJOY's own `'  '//dater()` padding, which a
    // trimmed token loses -- and losing it makes a read-then-write round trip
    // differ in the header while every value matches.
    let date_raw: Vec<u8> = l0
        .as_bytes()
        .get(hz_len + 25..hz_len + 35)
        .map(|b| b.to_vec())
        .unwrap_or_else(|| rest.get(2).unwrap_or(&"").as_bytes().to_vec());
    let date = String::from_utf8_lossy(&date_raw).trim().to_string();
    let (zaid_num, class) = split_zaid(&zaid)?;

    // Line 2: 70-column comment then a 10-column material id. Sliced, never
    // whitespace-split — the comment legitimately contains spaces.
    let l1 = lines[1];
    let comment_raw: Vec<u8> = l1.bytes().take(70).collect();
    let mat_raw: Vec<u8> = l1.bytes().skip(70).take(10).collect();
    let comment = String::from_utf8_lossy(&comment_raw).trim_end().to_string();
    let mat_id = String::from_utf8_lossy(&mat_raw).trim().to_string();

    // Lines 3-6: 16 IZ/AW pairs.
    let pair_toks: Vec<&str> = lines[2..6].iter().flat_map(|l| l.split_whitespace()).collect();
    if pair_toks.len() < 32 {
        return Err(NjoyError::EndfParse(format!(
            "ACE IZ/AW block: expected 32 values, found {}",
            pair_toks.len()
        )));
    }
    // STRICT, deliberately. An earlier draft used `unwrap_or(0)` here and in the
    // NXS/JXS/XSS parses below. That turns a corrupt token into a silent zero,
    // which is the worst possible failure for a comparison tool: the table
    // still "reads", the numbers still look plausible, and the difference gets
    // attributed to physics. A file that will not parse must say so.
    let mut iz = [0i32; 16];
    let mut aw = [0f64; 16];
    for k in 0..16 {
        iz[k] = pair_toks[2 * k].parse().map_err(|e| {
            NjoyError::EndfParse(format!("ACE IZ[{k}] {:?}: {e}", pair_toks[2 * k]))
        })?;
        aw[k] = pair_toks[2 * k + 1].parse().map_err(|e| {
            NjoyError::EndfParse(format!("ACE AW[{k}] {:?}: {e}", pair_toks[2 * k + 1]))
        })?;
    }

    // Lines 7-12: NXS(16) then JXS(32).
    let ints: Vec<i32> = lines[6..12]
        .iter()
        .flat_map(|l| l.split_whitespace())
        .map(|t| {
            t.parse::<i32>()
                .map_err(|e| NjoyError::EndfParse(format!("ACE NXS/JXS {t:?}: {e}")))
        })
        .collect::<Result<Vec<i32>, NjoyError>>()?;
    if ints.len() < 48 {
        return Err(NjoyError::EndfParse(format!(
            "ACE NXS/JXS: expected 48 integers, found {}",
            ints.len()
        )));
    }
    let mut nxs = [0i32; 16];
    let mut jxs = [0i32; 32];
    nxs.copy_from_slice(&ints[..16]);
    jxs.copy_from_slice(&ints[16..48]);

    let (xss, next) = take_xss(lines, 12, nxs[0])?;

    Ok((RawAceTable {
        file_type: AceFileType::Type1Ascii,
        header: AceHeader {
            zaid,
            zaid_num,
            class,
            awr,
            kt_mev,
            date: date.clone(),
            comment,
            mat_id,
            iz,
            aw,
            raw_text: [zaid_raw, date_raw, comment_raw, mat_raw],
        },
        nxs,
        jxs,
        xss,
        xss_is_int: None,
    }, start + next))
}


/// Read `NXS(1)` XSS values starting at `lines[first]`, returning them and the
/// index of the line after the last one. `NXS(1) = 0` (a hand-built table that
/// does not declare its length) takes every remaining value, as before.
///
/// NXS(1) is the declared XSS length. A file that runs out first is truncated,
/// which is worth an error rather than a silent short read -- a truncated ACE
/// is exactly what NJOY leaves behind when `change` fails partway. A table
/// whose last value is not at the end of a line cannot be followed by another
/// table, so that is an error too.
fn take_xss(lines: &[&str], first: usize, nxs1: i32) -> Result<(Vec<f64>, usize), NjoyError> {
    let want = if nxs1 > 0 { Some(nxs1 as usize) } else { None };
    let mut xss = Vec::with_capacity(want.unwrap_or(0));
    let mut i = first;
    while i < lines.len() {
        if want.is_some_and(|w| xss.len() >= w) {
            break;
        }
        for t in lines[i].split_whitespace() {
            let v = parse_ace_float(t)
                .ok_or_else(|| NjoyError::EndfParse(format!("ACE XSS value {t:?}")))?;
            xss.push(v);
        }
        i += 1;
    }
    if let Some(w) = want {
        if xss.len() != w {
            return Err(NjoyError::EndfParse(format!(
                "ACE XSS length: NXS(1) declares {w} values but the file holds {}. A \
                 truncated ACE is what upstream leaves behind when its Type-1 writer \
                 stops partway; a table ending mid-line cannot be followed by another.",
                xss.len()
            )));
        }
    }
    Ok((xss, i))
}

/// The ACE 2.0 header variant of [`parse_type1_at`]; `lines[0]` is the
/// `version name source` line. See [`parse_type1_library`].
fn parse_type1_v2(lines: &[&str]) -> Result<(RawAceTable, usize), NjoyError> {
    let w0: Vec<&str> = lines[0].split_whitespace().collect();
    let zaid = w0
        .get(1)
        .ok_or_else(|| NjoyError::EndfParse(format!("ACE 2.0 header line 1: no name in {:?}", lines[0])))?
        .to_string();
    let w1: Vec<&str> = lines[1].split_whitespace().collect();
    if w1.len() < 4 {
        return Err(NjoyError::EndfParse(format!(
            "ACE 2.0 header line 2: expected `awr kT date n`, got {:?}",
            lines[1]
        )));
    }
    let num = |t: &str, what: &str| {
        t.parse::<f64>()
            .map_err(|e| NjoyError::EndfParse(format!("ACE 2.0 {what} {t:?}: {e}")))
    };
    let awr = num(w1[0], "awr")?;
    let kt_mev = num(w1[1], "kT")?;
    let date = w1[2].to_string();
    let n_comment: usize = w1[3]
        .parse()
        .map_err(|e| NjoyError::EndfParse(format!("ACE 2.0 comment count {:?}: {e}", w1[3])))?;
    let h = 2 + n_comment; // first IZ/AW line
    if lines.len() < h + 10 {
        return Err(NjoyError::EndfParse("ACE 2.0 header runs past the end of the file".into()));
    }
    let comment = lines[2..h].iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
    let pair_toks: Vec<&str> = lines[h..h + 4].iter().flat_map(|l| l.split_whitespace()).collect();
    if pair_toks.len() < 32 {
        return Err(NjoyError::EndfParse(format!(
            "ACE 2.0 IZ/AW block: expected 32 values, found {}",
            pair_toks.len()
        )));
    }
    let mut iz = [0i32; 16];
    let mut aw = [0f64; 16];
    for k in 0..16 {
        iz[k] = pair_toks[2 * k]
            .parse()
            .map_err(|e| NjoyError::EndfParse(format!("ACE IZ[{k}]: {e}")))?;
        aw[k] = num(pair_toks[2 * k + 1], "AW")?;
    }
    let ints: Vec<i32> = lines[h + 4..h + 10]
        .iter()
        .flat_map(|l| l.split_whitespace())
        .map(|t| t.parse::<i32>().map_err(|e| NjoyError::EndfParse(format!("ACE NXS/JXS {t:?}: {e}"))))
        .collect::<Result<Vec<i32>, NjoyError>>()?;
    if ints.len() < 48 {
        return Err(NjoyError::EndfParse(format!(
            "ACE 2.0 NXS/JXS: expected 48 integers, found {}",
            ints.len()
        )));
    }
    let mut nxs = [0i32; 16];
    let mut jxs = [0i32; 32];
    nxs.copy_from_slice(&ints[..16]);
    jxs.copy_from_slice(&ints[16..48]);
    let (xss, next) = take_xss(lines, h + 10, nxs[0])?;
    let (zaid_num, class) = split_zaid(&zaid)?;
    let pad = |s: &str, n: usize| {
        let mut b: Vec<u8> = s.bytes().take(n).collect();
        b.resize(n, b' ');
        b
    };
    Ok((
        RawAceTable {
            file_type: AceFileType::Type1Ascii,
            header: AceHeader {
                raw_text: [pad(&zaid, 10), pad(&date, 10), pad(&comment, 70), pad("", 10)],
                zaid,
                zaid_num,
                class,
                awr,
                kt_mev,
                date,
                comment,
                mat_id: String::new(),
                iz,
                aw,
            },
            nxs,
            jxs,
            xss,
            xss_is_int: None,
        },
        next,
    ))
}

// ── Type 2: Fortran unformatted sequential ──────────────────────────────────

/// One f64 per XSS value.
const XSS_BYTES: usize = 8;

/// `ner` — XSS values per Type-2 data record, **which is not the same for
/// every ACE class**.
///
/// The fast/charged-particle writer blocks the data 512 reals to a record
/// (`acefc.f90:187`, `ner = 512`). Every other writer uses **one real per
/// record**: thermal (`aceth.f90:571`, and again at `:2302`), photoatomic
/// (`acepa.f90:297`, `:931`), dosimetry (`acedo.f90:319`, `:495`) and
/// photonuclear (`acepn.f90:1877`, `:2481`) all declare `ner = 1`.
///
/// This matters only for **writing**: a Fortran unformatted record carries its
/// own length, so [`read_type2`] recovers `xss` whatever the blocking is. A
/// writer that guessed 512 for a thermal or photoatomic table would produce a
/// file with the same values and different bytes.
///
/// Measured 2026-09-21 on NJOY's own Type-2 photoatomic output for the
/// synthetic Z=6 tape: a 500-byte header record followed by 449 records of
/// exactly 8 bytes each, total 7692 bytes — one real per record.
pub(crate) fn xss_per_record(class: AceClass) -> usize {
    match class {
        AceClass::ContinuousNeutron | AceClass::ChargedParticle(_) => 512,
        AceClass::Thermal
        | AceClass::Photoatomic
        | AceClass::Photonuclear
        | AceClass::Dosimetry => 1,
    }
}

/// Read one Fortran unformatted sequential record.
///
/// gfortran (and every compiler NJOY is built with here) brackets each record
/// with a 4-byte little-endian length marker, identical before and after. The
/// trailing copy is verified rather than skipped: a mismatch is the clearest
/// possible signal that the file is not actually an unformatted sequential
/// stream, and catching it here beats decoding garbage into an NXS array.
fn read_record(bytes: &[u8], pos: &mut usize) -> Result<Vec<u8>, NjoyError> {
    if *pos + 4 > bytes.len() {
        return Err(NjoyError::EndfParse(
            "ACE Type 2: file ends where a record marker was expected".into(),
        ));
    }
    let head = u32::from_le_bytes([bytes[*pos], bytes[*pos + 1], bytes[*pos + 2], bytes[*pos + 3]])
        as usize;
    *pos += 4;
    if *pos + head + 4 > bytes.len() {
        return Err(NjoyError::EndfParse(format!(
            "ACE Type 2: record declares {head} bytes but only {} remain",
            bytes.len().saturating_sub(*pos)
        )));
    }
    let body = bytes[*pos..*pos + head].to_vec();
    *pos += head;
    let tail = u32::from_le_bytes([bytes[*pos], bytes[*pos + 1], bytes[*pos + 2], bytes[*pos + 3]])
        as usize;
    *pos += 4;
    if head != tail {
        return Err(NjoyError::EndfParse(format!(
            "ACE Type 2: record markers disagree ({head} leading, {tail} trailing) — \
             this is not a Fortran unformatted sequential file"
        )));
    }
    Ok(body)
}

fn take_f64(b: &[u8], o: &mut usize) -> f64 {
    let v = f64::from_le_bytes(b[*o..*o + 8].try_into().expect("8 bytes"));
    *o += 8;
    v
}

fn take_i32(b: &[u8], o: &mut usize) -> i32 {
    let v = i32::from_le_bytes(b[*o..*o + 4].try_into().expect("4 bytes"));
    *o += 4;
    v
}


/// Read a **Type 2** (binary) ACE file — upstream `acer`'s `iopt = 8`.
///
/// Layout, from the writer at `acefc.f90:13028-13053`:
///
/// ```text
/// record 1   hz(10 or 13), aw0, tz, hd(10), hk(70), hm(10),
///            (izn(i) integer, awn(i) real, i=1,16),
///            NXS(16) as named scalars, JXS(32) as named scalars
/// record 2+  XSS in chunks of ner = 512 reals
/// ```
///
/// `mcnpx` widens `hz` from 10 to 13 characters; which one a file uses is not
/// recorded anywhere inside it, so the header length is inferred from the
/// record size — the rest of record 1 is fixed at 100 characters of text plus
/// 16 (int, real) pairs plus 48 integers.
pub fn read_type2<P: AsRef<Path>>(path: P) -> Result<RawAceTable, NjoyError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .map_err(|e| NjoyError::EndfParse(format!("read {}: {e}", path.display())))?;
    let mut tables = parse_type2_library(&bytes)?;
    match tables.len() {
        1 => Ok(tables.pop().expect("length checked")),
        n => Err(NjoyError::EndfParse(format!(
            "ACE Type 2 {}: the file holds {n} tables; read one by name with \
             `acer::read::read_table`",
            path.display()
        ))),
    }
}

/// Every Type-2 table in `bytes`, in file order — the binary counterpart of
/// [`parse_type1_library`] (`openmc/data/ace.py:243-325` reads Type-2 libraries
/// table after table the same way). GitHub #365 audit.
pub fn parse_type2_library(bytes: &[u8]) -> Result<Vec<RawAceTable>, NjoyError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        out.push(parse_type2_at(bytes, &mut pos)?);
    }
    if out.is_empty() {
        return Err(NjoyError::EndfParse("ACE Type 2: empty file".into()));
    }
    Ok(out)
}

/// Parse one Type-2 table starting at byte `*pos`, advancing it past the
/// table's last data record.
fn parse_type2_at(bytes: &[u8], pos: &mut usize) -> Result<RawAceTable, NjoyError> {
    let head = read_record(bytes, pos)?;

    // Everything after hz is fixed width, so hz's width follows from the record
    // length rather than from a flag the file does not carry.
    const FIXED: usize = 8 + 8                 // aw0, tz
        + 10 + 70 + 10                          // hd, hk, hm
        + 16 * (4 + 8)                          // izn/awn pairs
        + 16 * 4 + 32 * 4; // NXS, JXS
    let hz_len = head.len().checked_sub(FIXED).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "ACE Type 2: header record is {} bytes, shorter than the {FIXED} bytes \
             that follow the ZAID",
            head.len()
        ))
    })?;
    if hz_len != 10 && hz_len != 13 {
        return Err(NjoyError::EndfParse(format!(
            "ACE Type 2: implied ZAID width {hz_len}, expected 10 (standard) or 13 (mcnpx)"
        )));
    }

    let mut o = 0usize;
    let zaid_raw = head[o..o + hz_len].to_vec();
    o += hz_len;
    let zaid = String::from_utf8_lossy(&zaid_raw).trim().to_string();
    let awr = take_f64(&head, &mut o);
    let kt_mev = take_f64(&head, &mut o);
    let date_raw = head[o..o + 10].to_vec();
    o += 10;
    let comment_raw = head[o..o + 70].to_vec();
    o += 70;
    let mat_raw = head[o..o + 10].to_vec();
    o += 10;
    let date = String::from_utf8_lossy(&date_raw).trim().to_string();
    let comment = String::from_utf8_lossy(&comment_raw).trim_end().to_string();
    let mat_id = String::from_utf8_lossy(&mat_raw).trim().to_string();
    let mut iz = [0i32; 16];
    let mut aw = [0f64; 16];
    for k in 0..16 {
        iz[k] = take_i32(&head, &mut o);
        aw[k] = take_f64(&head, &mut o);
    }
    let mut nxs = [0i32; 16];
    for v in nxs.iter_mut() {
        *v = take_i32(&head, &mut o);
    }
    let mut jxs = [0i32; 32];
    for v in jxs.iter_mut() {
        *v = take_i32(&head, &mut o);
    }
    let (zaid_num, class) = split_zaid(&zaid)?;

    // Data records: ner = 512 reals each, the last one short.
    let want = nxs[0].max(0) as usize;
    let mut xss = Vec::with_capacity(want);
    while xss.len() < want {
        let rec = read_record(bytes, pos)?;
        if rec.len() % XSS_BYTES != 0 {
            return Err(NjoyError::EndfParse(format!(
                "ACE Type 2: data record of {} bytes is not a whole number of f64",
                rec.len()
            )));
        }
        let mut ro = 0usize;
        for _ in 0..rec.len() / XSS_BYTES {
            xss.push(take_f64(&rec, &mut ro));
        }
        let ner = xss_per_record(class);
        if rec.len() / XSS_BYTES > ner {
            return Err(NjoyError::EndfParse(format!(
                "ACE Type 2: data record holds {} reals, more than ner = {ner} for a \
                 {class:?} table",
                rec.len() / XSS_BYTES
            )));
        }
    }
    xss.truncate(want);

    Ok(RawAceTable {
        file_type: AceFileType::Type2Binary,
        header: AceHeader {
            zaid,
            zaid_num,
            class,
            awr,
            kt_mev,
            date,
            comment,
            mat_id,
            iz,
            aw,
            raw_text: [zaid_raw, date_raw, comment_raw, mat_raw],
        },
        nxs,
        jxs,
        xss,
        xss_is_int: None,
    })
}

/// Read an ACE file of either type, deciding which by looking at it.
///
/// A Type-1 file begins with printable text; a Type-2 file begins with a 4-byte
/// record marker whose value is the header record's length. Sniffing beats
/// asking the caller, because upstream itself distinguishes the two only by
/// which `iopt` the user typed — and a user who types the wrong one gets
/// garbage rather than a diagnostic.
pub fn read<P: AsRef<Path>>(path: P) -> Result<RawAceTable, NjoyError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .map_err(|e| NjoyError::EndfParse(format!("read {}: {e}", path.display())))?;

    // **gzip is transparent.** The `reference-data/ace` submodule stores every
    // table gzipped -- a U-235 ACE table is 136 MB raw and 23 MB compressed,
    // which is why they are stored that way and why a reader that cannot
    // inflate cannot read the reference library at all.
    let mut tables = read_library_bytes(bytes, path)?;
    match tables.len() {
        1 => Ok(tables.pop().expect("length checked")),
        n => Err(NjoyError::EndfParse(format!(
            "{}: the file holds {n} ACE tables ({}); read one by name with \
             `acer::read::read_table`, or all with `read_library`",
            path.display(),
            tables.iter().map(|t| t.header.zaid.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// Every ACE table in the file at `path`, of either type, gzipped or not, in
/// file order. GitHub #365 audit: an MCNP/OpenMC-style library concatenates
/// tables, and OpenMC's `Library` reads them all (`openmc/data/ace.py:194-241`).
pub fn read_library<P: AsRef<Path>>(path: P) -> Result<Vec<RawAceTable>, NjoyError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .map_err(|e| NjoyError::EndfParse(format!("read {}: {e}", path.display())))?;
    read_library_bytes(bytes, path)
}

/// The table named `name` (its ZAID / thermal name as the header writes it,
/// e.g. `92235.00c`, `92235.800nc`, `hh2o.01t`) from the file at `path` — the
/// equivalent of OpenMC's `ace.get_table(filename, name)`
/// (`openmc/data/ace.py:161-183`).
pub fn read_table<P: AsRef<Path>>(path: P, name: &str) -> Result<RawAceTable, NjoyError> {
    let path = path.as_ref();
    let tables = read_library(path)?;
    let names: Vec<String> = tables.iter().map(|t| t.header.zaid.clone()).collect();
    tables
        .into_iter()
        .find(|t| t.header.zaid.trim() == name.trim())
        .ok_or_else(|| {
            NjoyError::EndfParse(format!(
                "{}: no ACE table named {name:?}; the file holds {}",
                path.display(),
                names.join(", ")
            ))
        })
}

/// Sniff, inflate if gzipped, and parse every table. **gzip is transparent for
/// both types** (since 2026-09-29 a gzipped Type-2 file too; before, a gzip
/// member was always parsed as Type 1).
fn read_library_bytes(bytes: Vec<u8>, path: &Path) -> Result<Vec<RawAceTable>, NjoyError> {
    let bytes = if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        gunzip(&bytes, path)?
    } else {
        bytes
    };
    // Three layouts, told apart by their first bytes:
    // - Type 2 as NJOY writes it (Fortran *sequential* unformatted): a 4-byte
    //   record marker holding the header record's length, 500 or 503.
    // - Type 2 as MCNP and OpenMC read it (*direct access*, 4096-byte records,
    //   no markers): 10 characters of ZAID, then binary doubles.
    // - Type 1: text throughout.
    let marker = bytes
        .get(..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if matches!(marker, Some(500) | Some(503)) {
        return parse_type2_library(&bytes);
    }
    let printable = |b: &u8| b.is_ascii_graphic() || b.is_ascii_whitespace();
    let looks_direct = bytes.len() >= DIRECT_RECL
        && bytes[..10].iter().all(printable)
        && !bytes[10..26].iter().all(printable);
    if looks_direct {
        return parse_type2_direct_library(&bytes);
    }
    if bytes.len() >= 4 && bytes[..4].iter().all(printable) {
        let text = String::from_utf8(bytes).map_err(|e| {
            NjoyError::EndfParse(format!("{}: ACE text is not UTF-8: {e}", path.display()))
        })?;
        parse_type1_library(&text)
    } else {
        parse_type2_library(&bytes)
    }
}

/// Record length \[bytes\] of a direct-access Type-2 ACE file: 512 doubles,
/// OpenMC's `recl_length = 4096` / `entries = 512` (`openmc/data/ace.py:243`).
const DIRECT_RECL: usize = 4096;

/// Every table of a **direct-access** Type-2 ACE file — the binary layout MCNP
/// and OpenMC read, as distinct from the Fortran-sequential layout NJOY's ACER
/// writes (which [`parse_type2_library`] reads). GitHub #365 audit.
///
/// A port of OpenMC's `Library._read_binary` (`openmc/data/ace.py:243-325`):
/// record 1 holds `name(10) awr kT date(10) comment(70) mat(10)`, 16
/// `(izaid i32, awr f64)` pairs, NXS(16) and JXS(32) as i32, little-endian;
/// XSS starts at the next 4096-byte record and runs for `NXS(1)` doubles; the
/// next table starts at record `ceil(NXS(1)/512) + 1`. OpenMC's reader cannot
/// read NJOY's sequential Type 2 (it fails decoding the record marker as the
/// name — observed on the five-route study's `B10.ace`), and until this port
/// this reader could not read the direct-access one.
pub fn parse_type2_direct_library(bytes: &[u8]) -> Result<Vec<RawAceTable>, NjoyError> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        if bytes.len() - start < 500 {
            return Err(NjoyError::EndfParse(format!(
                "ACE Type 2 (direct access): {} trailing bytes cannot hold a header",
                bytes.len() - start
            )));
        }
        let head = &bytes[start..start + 500];
        let mut o = 0usize;
        let zaid_raw = head[..10].to_vec();
        o += 10;
        let zaid = String::from_utf8_lossy(&zaid_raw).trim().to_string();
        let awr = take_f64(head, &mut o);
        let kt_mev = take_f64(head, &mut o);
        let date_raw = head[o..o + 10].to_vec();
        o += 10;
        let comment_raw = head[o..o + 70].to_vec();
        o += 70;
        let mat_raw = head[o..o + 10].to_vec();
        o += 10;
        let mut iz = [0i32; 16];
        let mut aw = [0f64; 16];
        for k in 0..16 {
            iz[k] = take_i32(head, &mut o);
            aw[k] = take_f64(head, &mut o);
        }
        let mut nxs = [0i32; 16];
        for v in nxs.iter_mut() {
            *v = take_i32(head, &mut o);
        }
        let mut jxs = [0i32; 32];
        for v in jxs.iter_mut() {
            *v = take_i32(head, &mut o);
        }
        let n = nxs[0].max(0) as usize;
        let x0 = start + DIRECT_RECL;
        if x0 + n * XSS_BYTES > bytes.len() {
            return Err(NjoyError::EndfParse(format!(
                "ACE Type 2 (direct access) {zaid}: NXS(1) = {n} runs past the end of the file"
            )));
        }
        let mut xo = x0;
        let xss: Vec<f64> = (0..n).map(|_| take_f64(bytes, &mut xo)).collect();
        let (zaid_num, class) = split_zaid(&zaid)?;
        out.push(RawAceTable {
            file_type: AceFileType::Type2Binary,
            header: AceHeader {
                zaid,
                zaid_num,
                class,
                awr,
                kt_mev,
                date: String::from_utf8_lossy(&date_raw).trim().to_string(),
                comment: String::from_utf8_lossy(&comment_raw).trim_end().to_string(),
                mat_id: String::from_utf8_lossy(&mat_raw).trim().to_string(),
                iz,
                aw,
                raw_text: [zaid_raw, date_raw, comment_raw, mat_raw],
            },
            nxs,
            jxs,
            xss,
            xss_is_int: None,
        });
        start += DIRECT_RECL * (n.div_ceil(512) + 1);
    }
    Ok(out)
}

/// Inflate a gzip member to a `String`.
///
/// `miniz_oxide` implements raw DEFLATE and zlib, **not** gzip, so the
/// container has to be unwrapped here: a 10-byte fixed header, the optional
/// FEXTRA/FNAME/FCOMMENT/FHCRC fields named by the flag byte, the DEFLATE
/// stream, and an 8-byte trailer whose second word is the uncompressed size
/// mod 2^32 (RFC 1952 sections 2.2-2.3).
///
/// That trailer is used as the inflate **limit** rather than trusted as the
/// answer: it bounds the allocation for a hostile or corrupt file, while a
/// short read is still caught by the decoder returning fewer bytes.
fn gunzip(bytes: &[u8], path: &Path) -> Result<Vec<u8>, NjoyError> {
    let bad = |m: String| NjoyError::EndfParse(format!("{}: {m}", path.display()));
    if bytes.len() < 18 {
        return Err(bad("gzip file is too short to hold a header and trailer".into()));
    }
    if bytes[2] != 8 {
        return Err(bad(format!(
            "gzip compression method {} is not DEFLATE",
            bytes[2]
        )));
    }
    let flg = bytes[3];
    let mut i = 10usize;
    if flg & 0b0000_0100 != 0 {
        // FEXTRA: two-byte length, then that many bytes.
        if i + 2 > bytes.len() {
            return Err(bad("gzip FEXTRA runs past end of file".into()));
        }
        let xlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2 + xlen;
    }
    for (bit, what) in [(0b0000_1000u8, "FNAME"), (0b0001_0000, "FCOMMENT")] {
        if flg & bit != 0 {
            // NUL-terminated string.
            let start = i;
            while i < bytes.len() && bytes[i] != 0 {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(bad(format!("gzip {what} is unterminated (from byte {start})")));
            }
            i += 1;
        }
    }
    if flg & 0b0000_0010 != 0 {
        i += 2; // FHCRC
    }
    if i + 8 > bytes.len() {
        return Err(bad("gzip header runs into the trailer".into()));
    }
    let n = bytes.len();
    let isize_le = u32::from_le_bytes([bytes[n - 4], bytes[n - 3], bytes[n - 2], bytes[n - 1]]);
    // A 4 GiB-and-over member wraps ISIZE to a small number, which would cap
    // the inflate far too low. Nothing in this library is near that, so the
    // floor keeps a wrapped value from silently truncating instead of failing.
    let limit = (isize_le as usize).max(1 << 20);
    let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[i..n - 8], limit)
        .map_err(|e| bad(format!("gzip inflate failed: {e:?}")))?;
    if raw.len() as u32 != isize_le {
        return Err(bad(format!(
            "gzip inflated {} bytes but the trailer declares {isize_le}",
            raw.len()
        )));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny legacy-header Type-1 table with `n` XSS values `1..=n`, written
    /// 4 per line, NXS(1) = n. For the layout tests below (GitHub #365 audit).
    fn tiny_table(zaid: &str, n: usize) -> String {
        let mut s = format!("{zaid:>10}    0.999167  2.5301E-08   09/29/26\n");
        s += &format!("{:<70}{:>10}\n", "tiny", "mat125");
        for _ in 0..4 {
            s += "      0         0.      0         0.      0         0.      0         0.\n";
        }
        let mut ints = vec![0i32; 48];
        ints[0] = n as i32;
        ints[1] = 1001;
        for row in ints.chunks(8) {
            s += &row.iter().map(|v| format!("{v:>9}")).collect::<String>();
            s += "\n";
        }
        for row in (1..=n).collect::<Vec<_>>().chunks(4) {
            s += &row.iter().map(|v| format!("{:>20.11E}", *v as f64)).collect::<String>();
            s += "\n";
        }
        s
    }

    #[test]
    fn a_library_of_type1_tables_is_read_table_by_table() {
        let text = tiny_table("1001.00c", 6) + &tiny_table("1002.00c", 9);
        let tabs = parse_type1_library(&text).expect("library");
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].header.zaid, "1001.00c");
        assert_eq!(tabs[0].xss, (1..=6).map(|v| v as f64).collect::<Vec<_>>());
        assert_eq!(tabs[1].header.zaid, "1002.00c");
        assert_eq!(tabs[1].xss.len(), 9);
        let err = parse_type1(&text).unwrap_err().to_string();
        assert!(err.contains("2 tables") && err.contains("1002.00c"), "{err}");
    }

    #[test]
    fn the_ace_2_header_is_read() {
        let legacy = tiny_table("1001.00c", 5);
        let body: Vec<&str> = legacy.lines().skip(2).collect();
        let text = format!(
            "2.0.1          1001.800nc ENDF/B-VIII.0\n    0.999167  2.5301E-08 09/29/26 2\nc1\nc2\n{}\n",
            body.join("\n")
        );
        let t = parse_type1(&text).expect("2.0 header");
        assert_eq!(t.header.zaid, "1001.800nc");
        assert_eq!(t.header.class, AceClass::ContinuousNeutron);
        assert_eq!(t.header.awr, 0.999167);
        assert_eq!(t.header.comment, "c1 c2");
        assert_eq!(t.xss, parse_type1(&legacy).unwrap().xss);
    }

    #[test]
    fn exponentless_floats_are_read_as_upstream_writes_them() {
        assert_eq!(parse_ace_float("1.2345-120"), Some(1.2345e-120));
        assert_eq!(parse_ace_float("-1.2345+101"), Some(-1.2345e101));
        assert_eq!(parse_ace_float("1.0E-05"), Some(1.0e-5));
        assert_eq!(parse_ace_float("-3.5"), Some(-3.5));
        assert_eq!(parse_ace_float("abc"), None);
    }

    #[test]
    fn a_direct_access_type2_library_is_read() {
        // Two tables in OpenMC's direct-access layout (ace.py:243-325).
        let mut bytes = Vec::new();
        for (name, n) in [("1001.00c", 3usize), ("1002.00c", 600)] {
            let mut head = Vec::new();
            head.extend_from_slice(format!("{name:>10}").as_bytes());
            head.extend_from_slice(&0.999167f64.to_le_bytes());
            head.extend_from_slice(&2.53e-8f64.to_le_bytes());
            head.extend_from_slice(&[b' '; 90]);
            for _ in 0..16 {
                head.extend_from_slice(&0i32.to_le_bytes());
                head.extend_from_slice(&0f64.to_le_bytes());
            }
            for k in 0..48 {
                head.extend_from_slice(&(if k == 0 { n as i32 } else { 0 }).to_le_bytes());
            }
            assert_eq!(head.len(), 500);
            head.resize(DIRECT_RECL, 0);
            bytes.extend_from_slice(&head);
            let mut body: Vec<u8> = (1..=n).flat_map(|v| (v as f64).to_le_bytes()).collect();
            body.resize(8 * 512 * n.div_ceil(512), 0);
            bytes.extend_from_slice(&body);
        }
        let tabs = parse_type2_direct_library(&bytes).expect("direct access");
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].xss, vec![1.0, 2.0, 3.0]);
        assert_eq!(tabs[1].header.zaid, "1002.00c");
        assert_eq!(tabs[1].xss.len(), 600);
        assert_eq!(tabs[1].xss[599], 600.0);
    }

    #[test]
    fn thermal_zaid_has_no_numeric_form() {
        let (num, class) = split_zaid("  al27.00t").unwrap();
        assert_eq!(class, AceClass::Thermal);
        assert!(
            num.is_none(),
            "a thermal ZAID is a name; upstream skips the numeric read for it"
        );
    }

    #[test]
    fn continuous_energy_zaid_parses_numerically() {
        let (num, class) = split_zaid("92235.00c").unwrap();
        assert_eq!(class, AceClass::ContinuousNeutron);
        assert_eq!(num, Some(92235.00));
    }

    #[test]
    fn incident_alpha_is_a_charged_particle_class() {
        let (num, class) = split_zaid("2004.00a").unwrap();
        assert_eq!(class, AceClass::ChargedParticle('a'));
        assert_eq!(num, Some(2004.00));
    }

    /// Every letter upstream dispatches on is recognised, and nothing else is.
    #[test]
    fn class_letters_match_upstream_dispatch() {
        for (c, want) in [
            ('c', AceClass::ContinuousNeutron),
            ('h', AceClass::ChargedParticle('h')),
            ('o', AceClass::ChargedParticle('o')),
            ('r', AceClass::ChargedParticle('r')),
            ('s', AceClass::ChargedParticle('s')),
            ('a', AceClass::ChargedParticle('a')),
            ('t', AceClass::Thermal),
            ('p', AceClass::Photoatomic),
            ('u', AceClass::Photonuclear),
            ('y', AceClass::Dosimetry),
        ] {
            assert_eq!(AceClass::from_letter(c), Some(want), "letter {c:?}");
            assert_eq!(want.letter(), c);
        }
        for c in ['x', 'z', '0', 'C'] {
            assert!(AceClass::from_letter(c).is_none(), "letter {c:?} is not a class");
        }
    }

    /// A corrupt XSS token is an error, never a silent zero. This is the
    /// failure the strict parses above exist to prevent: a table that still
    /// "reads", with plausible numbers, whose difference gets blamed on physics.
    #[test]
    fn a_corrupt_value_is_an_error_not_a_silent_zero() {
        let mut lines: Vec<String> = Vec::new();
        lines.push("  1001.00c    0.999167  0.0000E+00   09/21/26".into());
        lines.push(format!("{:<70}{:>10}", "test", "mat125"));
        for _ in 0..4 {
            lines.push("      0         0.      0         0.      0         0.      0         0.".into());
        }
        lines.push("        3        0        0        0        0        0        0        0".into());
        lines.push("        0        0        0        0        0        0        0        0".into());
        for _ in 0..4 {
            lines.push("        1        0        0        0        0        0        0        0".into());
        }
        lines.push("   1.0000E+00   NOT_A_NUMBER   3.0000E+00".into());
        let e = parse_type1(&lines.join("\n")).unwrap_err();
        let msg = format!("{e}");
        assert!(
            msg.contains("NOT_A_NUMBER"),
            "the error must name the offending token, got: {msg}"
        );
    }

    #[test]
    fn unknown_class_letter_is_an_error_not_a_guess() {
        let e = split_zaid("92235.00z").unwrap_err();
        let msg = format!("{e}");
        assert!(msg.contains("class letters"), "message should name the valid set: {msg}");
    }
}

// ── Writing a table back out ────────────────────────────────────────────────

/// Pad or truncate `s` to exactly `n` bytes of ASCII, space-filled on the right.
///
/// Fortran `character(n)` fields are fixed width and space-padded; a shorter
/// Emit one Fortran unformatted sequential record: 4-byte length, body, length.
fn push_record(out: &mut Vec<u8>, body: &[u8]) {
    let n = body.len() as u32;
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(&n.to_le_bytes());
}

impl RawAceTable {
    /// Serialise as a **Type 2** (Fortran unformatted sequential) ACE file —
    /// the container upstream writes for `itype = 2`.
    ///
    /// Layout is the writer at `acefc.f90:13028-13053`:
    ///
    /// ```text
    /// record 1   hz(10), aw0, tz, hd(10), hk(70), hm(10),
    ///            (izn(i) integer, awn(i) real, i=1,16),
    ///            NXS(16), JXS(32)
    /// record 2+  XSS in chunks of ner reals -- 512 for a fast or
    ///            charged-particle table, 1 for every other class; see
    ///            [`xss_per_record`]
    /// ```
    ///
    /// The ZAID is written at the width it was read at (10, or 13 for mcnpx),
    /// so a table read from one file and written back produces the same record
    /// length rather than silently switching format.
    pub fn to_type2_bytes(&self) -> Vec<u8> {
        // Prefer the bytes as they were read. A field reconstructed from the
        // trimmed string plus a padding rule cannot reproduce NJOY's own output
        // -- Al-27's Type-2 `hz` ends in byte 0x0E, not a space -- and this
        // writer exists so that a read-then-write round trip is byte-exact.
        let raw_or = |i: usize, s: &str, n: usize| -> Vec<u8> {
            let r = &self.header.raw_text[i];
            if r.len() == n {
                r.clone()
            } else {
                fixed_field(s, n)
            }
        };
        let hz_len = if self.header.raw_text[0].len() == 13 { 13 } else { 10 };
        let mut head: Vec<u8> = Vec::with_capacity(500);
        head.extend_from_slice(&raw_or(0, &self.header.zaid, hz_len));
        head.extend_from_slice(&self.header.awr.to_le_bytes());
        head.extend_from_slice(&self.header.kt_mev.to_le_bytes());
        head.extend_from_slice(&raw_or(1, &self.header.date, 10));
        head.extend_from_slice(&raw_or(2, &self.header.comment, 70));
        head.extend_from_slice(&raw_or(3, &self.header.mat_id, 10));
        for k in 0..16 {
            head.extend_from_slice(&self.header.iz[k].to_le_bytes());
            head.extend_from_slice(&self.header.aw[k].to_le_bytes());
        }
        for v in self.nxs.iter() {
            head.extend_from_slice(&v.to_le_bytes());
        }
        for v in self.jxs.iter() {
            head.extend_from_slice(&v.to_le_bytes());
        }

        let mut out = Vec::with_capacity(head.len() + self.xss.len() * XSS_BYTES + 4096);
        push_record(&mut out, &head);
        for chunk in self.xss.chunks(xss_per_record(self.header.class)) {
            let mut body = Vec::with_capacity(chunk.len() * XSS_BYTES);
            for v in chunk {
                body.extend_from_slice(&v.to_le_bytes());
            }
            push_record(&mut out, &body);
        }
        out
    }

    /// Write this table to `path` as a Type-2 ACE file.
    pub fn write_type2<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        let path = path.as_ref();
        std::fs::write(path, self.to_type2_bytes())
            .map_err(|e| NjoyError::EndfParse(format!("write {}: {e}", path.display())))
    }
}

impl RawAceTable {
    /// The edits upstream's `iopt = 7/8` applies between reading a table and
    /// writing it back — `phofix` (`acepa.f90:344-368`), and the same three
    /// steps in `thrfix`, `dosfix`, `phnfix` and `acefix`.
    ///
    /// * `suffix` — replace the ZAID's fractional part: `ZA + suff`, written
    ///   `f9.2` then the class letter, or `f10.3` then the three-character
    ///   class string in the mcnpx variant. `None` leaves the ZAID alone, and
    ///   so does **any** suffix on a **thermal** table, whose ZAID is a name
    ///   and not a number (`acepa.f90:350-352` tests `ht(1:1) /= 't'`).
    ///   Upstream spells "leave it alone" as a negative `suff`; this port uses
    ///   `None`, because a negative suffix is not a value anyone means.
    /// * `comment` — replace `hk`. Upstream keeps the file's own comment when
    ///   the user supplies an empty one (`:361-363`), so an empty string here
    ///   is a no-op rather than a way to blank it.
    /// * `keep_iz_aw` — upstream's `nxtra /= 0` branch (`:364-368`), which
    ///   copies the **file's** IZ/AW pairs over the caller's. Since a table
    ///   read from a file already carries them, that is "keep"; `false`
    ///   clears them, which is what `nxtra = 0` writes.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when a suffix is asked for but the ZAID has no
    /// numeric part to rebuild it from.
    pub fn apply_edits(
        &mut self,
        suffix: Option<f64>,
        comment: Option<&str>,
        keep_iz_aw: bool,
    ) -> Result<(), NjoyError> {
        if let Some(suff) = suffix {
            if self.header.class != AceClass::Thermal {
                let za = self.header.zaid_num.ok_or_else(|| {
                    NjoyError::EndfParse(format!(
                        "apply_edits: ZAID {:?} has no numeric part, so a suffix \
                         cannot be applied",
                        self.header.zaid
                    ))
                })?;
                let raw = &self.header.raw_text[0];
                let mcnpx = raw.len() == 13;
                let class_str = if mcnpx {
                    String::from_utf8_lossy(&raw[10..13]).to_string()
                } else {
                    String::from_utf8_lossy(&raw[9..10]).to_string()
                };
                let zaid_num = za.round() + suff;
                let hz = if mcnpx {
                    format!("{zaid_num:10.3}{class_str}")
                } else {
                    format!("{zaid_num:9.2}{class_str}")
                };
                self.header.zaid = hz.trim().to_string();
                self.header.zaid_num = Some(zaid_num);
                self.header.raw_text[0] = hz.into_bytes();
            }
        }
        if let Some(hk) = comment {
            if !hk.trim().is_empty() {
                self.header.comment = hk.to_string();
                self.header.raw_text[2] = format!("{hk:<70}").into_bytes();
            }
        }
        if !keep_iz_aw {
            self.header.iz = [0; 16];
            self.header.aw = [0.0; 16];
        }
        Ok(())
    }

    /// Which `xss` words this table's class writes as integers, derived from
    /// `NXS`/`JXS` and the counts stored in `xss` itself.
    ///
    /// A file does not record the integer/real split, but for three of the
    /// classes it does not have to: the writer walks a layout that `NXS` and
    /// `JXS` fully describe, so the split can be **derived** rather than
    /// guessed. That matters for a read-then-write round trip, where the
    /// heuristic in [`to_type1_string`][Self::to_type1_string] gets an
    /// integral *value* wrong — a dosimetry cross section of exactly
    /// `1.00000000000E+00` comes back out as `1`.
    ///
    /// `None` for the continuous-energy and charged-particle classes, whose
    /// writer (`change`, `acefc.f90:13066-13200`) decides word by word over a
    /// far larger set of blocks; those keep the heuristic.
    pub fn derive_xss_is_int(&self) -> Option<Vec<bool>> {
        let n = self.xss.len();
        let mut mask = vec![false; n];
        let at = |loc: i32| -> usize { (loc.max(1) - 1) as usize };
        let count = |p: usize| -> usize { self.xss.get(p).copied().unwrap_or(0.0).max(0.0) as usize };
        match self.header.class {
            // `phoout` writes every word as `1pe20.11` (`acepa.f90:966-999`).
            AceClass::Photoatomic => Some(mask),
            // `dosout` (`acedo.f90:520-550`): MTR and LSIG are integers, and
            // inside SIGD so are `NR`, the `2*NR` region table and `NE`.
            AceClass::Dosimetry => {
                let ntr = self.nxs[3].max(0) as usize;
                for k in 0..ntr {
                    if let Some(m) = mask.get_mut(at(self.jxs[2]) + k) {
                        *m = true;
                    }
                    if let Some(m) = mask.get_mut(at(self.jxs[5]) + k) {
                        *m = true;
                    }
                }
                let mut p = at(self.jxs[6]);
                for _ in 0..ntr {
                    if p >= n {
                        break;
                    }
                    let nr = count(p);
                    mask[p] = true;
                    p += 1;
                    for _ in 0..2 * nr {
                        if p >= n {
                            break;
                        }
                        mask[p] = true;
                        p += 1;
                    }
                    if p >= n {
                        break;
                    }
                    let ne = count(p);
                    mask[p] = true;
                    p += 1 + 2 * ne;
                }
                Some(mask)
            }
            // `throut` (`aceth.f90:2327-2385`): each block's leading `NE` is an
            // integer, as is IFENG=2's `2*NE` locator/count table.
            AceClass::Thermal => {
                let itie = at(self.jxs[0]);
                let ne = count(itie);
                if itie < n {
                    mask[itie] = true;
                }
                if self.nxs[6] > 1 {
                    let itxe = at(self.jxs[2]);
                    for k in 0..2 * ne {
                        if let Some(m) = mask.get_mut(itxe + k) {
                            *m = true;
                        }
                    }
                }
                for slot in [3usize, 6] {
                    let loc = self.jxs[slot];
                    if loc > 0 {
                        if let Some(m) = mask.get_mut(at(loc)) {
                            *m = true;
                        }
                    }
                }
                Some(mask)
            }
            // `phnout` (`acepn.f90:2504-2780`) navigates by locator through a
            // per-emitted-particle structure, and every count it needs is
            // stored beside the data -- so the whole split is recoverable.
            AceClass::Photonuclear => {
                crate::acer::photonuclear::layout::int_mask(&self.nxs, &self.jxs, &self.xss)
            }
            _ => None,
        }
    }

    /// Serialise as a **Type 1** (ASCII) ACE file.
    ///
    /// The counterpart to [`to_type2_bytes`][Self::to_type2_bytes], so a table
    /// read from either container can be written back to either — which is
    /// what upstream's `iopt = 7/8` do (`acer.f90:485-533` reads, `itype`
    /// chooses the container it is written back as).
    ///
    /// **Integer-valued words are written as integers**, matching `change`
    /// (`acefc.f90:13088`). Which words those are is not recorded anywhere in
    /// the file, so it is inferred the only way available from a raw table: a
    /// value that is exactly integral and small enough to be a locator, count
    /// or MT number is written as one. That reproduces NJOY's own Type-1
    /// output on the tables tested, and the test says so rather than the doc
    /// claiming it in general.
    pub fn to_type1_string(&self) -> String {
        let mut out = String::with_capacity(self.xss.len() * 21 + 1024);
        let txt = |i: usize, fallback: &str, n: usize| -> String {
            let r = &self.header.raw_text[i];
            if r.len() == n {
                String::from_utf8_lossy(r).to_string()
            } else {
                format!("{fallback:<n$}")
            }
        };
        // The ZAID field is `a10` normally and `a13` in the mcnpx variant
        // (`acepa.f90:249-252`, `acedo.f90:504-511`, and the same pair in every
        // other writer). Which one this table uses is not a flag it carries --
        // it is the width of the field as stored, so take it from there rather
        // than assuming the common case.
        let hz_len = if self.header.raw_text[0].len() == 13 { 13 } else { 10 };
        out.push_str(&format!(
            "{}{} {} {}\n",
            txt(0, &self.header.zaid, hz_len),
            fortran_f(self.header.awr, 12, 6),
            fortran_e(self.header.kt_mev, 4, 11),
            txt(1, &self.header.date, 10),
        ));
        out.push_str(&format!(
            "{}{}\n",
            txt(2, &self.header.comment, 70),
            txt(3, &self.header.mat_id, 10)
        ));
        for row in 0..4 {
            let mut line = String::new();
            for col in 0..4 {
                let k = row * 4 + col;
                line.push_str(&format!("{:7}{}", self.header.iz[k], fortran_f0(self.header.aw[k])));
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        for (i, v) in self.nxs.iter().chain(self.jxs.iter()).enumerate() {
            out.push_str(&format!("{v:9}"));
            if i % 8 == 7 {
                out.push('\n');
            }
        }
        let derived = self.derive_xss_is_int();
        // Which words are written as integers is a property of the *class*,
        // because each class has its own writer. `phoout` (`acepa.f90:966-999`)
        // writes every photo-atomic word with `typen(l, nout, 2)`, i.e.
        // `1pe20.11`, so a zero there is `0.00000000000E+00` and not `0`.
        // A builder that knows word by word supplies `xss_is_int` instead.
        let all_real = self.header.class == AceClass::Photoatomic;
        for (i, v) in self.xss.iter().enumerate() {
            let as_int = match self.xss_is_int.as_ref().or(derived.as_ref()) {
                Some(mask) => mask.get(i).copied().unwrap_or(false),
                None => !all_real && v.fract() == 0.0 && v.abs() < 1.0e9,
            };
            if as_int {
                out.push_str(&fortran_i20(*v));
            } else {
                out.push_str(&fortran_e20(*v));
            }
            if i % 4 == 3 {
                out.push('\n');
            }
        }
        if self.xss.len() % 4 != 0 {
            out.push('\n');
        }
        out
    }
}

