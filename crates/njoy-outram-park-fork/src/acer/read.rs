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
    let body = &trimmed[..trimmed.len() - letter.len_utf8()];
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

    // Line 1: the ZAID is a fixed 10-column field; the rest is free-form.
    let l0 = lines[0];
    let zaid = l0.chars().take(10).collect::<String>().trim().to_string();
    let rest: Vec<&str> = l0.get(10..).unwrap_or("").split_whitespace().collect();
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
    let date = rest.get(2).unwrap_or(&"").to_string();
    let (zaid_num, class) = split_zaid(&zaid)?;

    // Line 2: 70-column comment then a 10-column material id. Sliced, never
    // whitespace-split — the comment legitimately contains spaces.
    let l1 = lines[1];
    let comment = l1.chars().take(70).collect::<String>().trim_end().to_string();
    let mat_id = l1.chars().skip(70).collect::<String>().trim().to_string();

    // Lines 3-6: 16 IZ/AW pairs.
    let pair_toks: Vec<&str> = lines[2..6].iter().flat_map(|l| l.split_whitespace()).collect();
    if pair_toks.len() < 32 {
        return Err(NjoyError::EndfParse(format!(
            "ACE IZ/AW block: expected 32 values, found {}",
            pair_toks.len()
        )));
    }
    let mut iz = [0i32; 16];
    let mut aw = [0f64; 16];
    for k in 0..16 {
        iz[k] = pair_toks[2 * k].parse().unwrap_or(0);
        aw[k] = pair_toks[2 * k + 1].parse().unwrap_or(0.0);
    }

    // Lines 7-12: NXS(16) then JXS(32).
    let ints: Vec<i32> = lines[6..12]
        .iter()
        .flat_map(|l| l.split_whitespace())
        .map(|t| t.parse::<i32>().unwrap_or(0))
        .collect();
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
        .map(|t| t.parse::<f64>().unwrap_or(0.0))
        .collect();

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
            date,
            comment,
            mat_id,
            iz,
            aw,
        },
        nxs,
        jxs,
        xss,
    })
}

// ── Type 2: Fortran unformatted sequential ──────────────────────────────────

/// One f64 per XSS value; `nbw = 8` in `acefc.f90:188`.
const XSS_BYTES: usize = 8;
/// `ner = 512` (`acefc.f90:187`) — XSS values per data record.
const XSS_PER_RECORD: usize = 512;

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

fn take_str(b: &[u8], o: &mut usize, n: usize) -> String {
    let s = String::from_utf8_lossy(&b[*o..*o + n]).to_string();
    *o += n;
    s
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
    let zaid = take_str(&head, &mut o, hz_len).trim().to_string();
    let awr = take_f64(&head, &mut o);
    let kt_mev = take_f64(&head, &mut o);
    let date = take_str(&head, &mut o, 10).trim().to_string();
    let comment = take_str(&head, &mut o, 70).trim_end().to_string();
    let mat_id = take_str(&head, &mut o, 10).trim().to_string();
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
        if rec.len() / XSS_BYTES > XSS_PER_RECORD {
            return Err(NjoyError::EndfParse(format!(
                "ACE Type 2: data record holds {} reals, more than ner = {XSS_PER_RECORD}",
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
        },
        nxs,
        jxs,
        xss,
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

    #[test]
    fn unknown_class_letter_is_an_error_not_a_guess() {
        let e = split_zaid("92235.00z").unwrap_err();
        let msg = format!("{e}");
        assert!(msg.contains("class letters"), "message should name the valid set: {msg}");
    }
}
