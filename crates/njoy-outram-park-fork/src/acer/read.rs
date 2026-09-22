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
pub fn parse_type1(text: &str) -> Result<RawAceTable, NjoyError> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 12 {
        return Err(NjoyError::EndfParse(format!(
            "ACE Type 1: {} lines, but the header alone is 12",
            lines.len()
        )));
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

    // The remainder is XSS.
    let xss: Vec<f64> = lines[12..]
        .iter()
        .flat_map(|l| l.split_whitespace())
        .map(|t| {
            t.parse::<f64>()
                .map_err(|e| NjoyError::EndfParse(format!("ACE XSS value {t:?}: {e}")))
        })
        .collect::<Result<Vec<f64>, NjoyError>>()?;

    // NXS(1) is the declared XSS length. Disagreement means a truncated file,
    // which is worth an error rather than a silent short read -- a truncated
    // ACE is exactly what NJOY leaves behind when `change` fails partway.
    let declared = nxs[0] as usize;
    if declared != 0 && declared != xss.len() {
        return Err(NjoyError::EndfParse(format!(
            "ACE XSS length: NXS(1) declares {declared} values but the file holds \
             {}. A truncated ACE is what upstream leaves behind when its Type-1 \
             writer stops partway.",
            xss.len()
        )));
    }

    Ok(RawAceTable {
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
    })
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
    let mut pos = 0usize;
    let head = read_record(&bytes, &mut pos)?;

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
        let rec = read_record(&bytes, &mut pos)?;
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
    let head = {
        let bytes = std::fs::read(path)
            .map_err(|e| NjoyError::EndfParse(format!("read {}: {e}", path.display())))?;
        bytes.into_iter().take(4).collect::<Vec<u8>>()
    };
    if head.len() == 4 && head.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        read_type1(path)
    } else {
        read_type2(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

