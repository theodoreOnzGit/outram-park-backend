//! ICU's Greek upper-casing (`toLocaleUpperCase("el")` in V8), for the port of
//! `CSL.toLocaleUpperCase` (load.js; GitHub #804).
//!
//! Hand port of ICU4C `GreekUpper::toUpper` (`i18n/ustrcase.cpp`; design note
//! <https://icu.unicode.org/design/case/greek-upper>): tonos, oxia, varia,
//! perispomeni and the breathing marks are dropped from upper-cased Greek
//! letters, a dialytika is added to an iota or upsilon that follows a vowel
//! whose accent was dropped, a ypogegrammeni becomes a trailing capital iota,
//! and a lone accented eta keeps its tonos (the disjunctive "or").
//!
//! **Why not `icu_casemap`'s own Greek path.** ICU4X's `CaseMapper` has a Greek
//! upper-casing path, but measured against node 22 (ICU 78) it differs: it
//! deletes a combining dialytika after a *consonant* (`β\u{308}` gives `Β`,
//! ICU gives `Β\u{308}`) and handles an already-accented vowel followed by
//! further combining accents differently. ICU4X is still used for everything
//! else (Greek and Lithuanian lower-casing, Turkic, Lithuanian upper-casing).
//!
//! The per-letter data ICU keeps in two hand-written tables (`data0370`,
//! `data1F00`) is derived here from the character's canonical decomposition
//! (base letter plus marks), which is what those tables encode.
//!
//! Result (`load.rs` `locale_case_tests`): 0 mismatches against node on the
//! differential set, see `DEVIATIONS.md` C5.

use icu_normalizer::DecomposingNormalizerBorrowed;
use icu_properties::props::{CaseIgnorable, Cased};
use icu_properties::CodePointSetData;

const VOWEL: u32 = 0x1000;
const YPOGEGRAMMENI: u32 = 0x2000;
const ACCENT: u32 = 0x4000;
/// Precomposed dialytika.
const DIALYTIKA: u32 = 0x8000;
const COMBINING_DIALYTIKA: u32 = 0x10000;
const EITHER_DIALYTIKA: u32 = DIALYTIKA | COMBINING_DIALYTIKA;

/// ICU `getLetterData`: `(upper-case base letter, flags)` for a Greek letter
/// (and `U+2126` OHM SIGN), `None` for anything ICU's tables give no data.
fn letter_data(c: char) -> Option<(char, u32)> {
    let cp = c as u32;
    if cp == 0x2126 {
        return Some(('\u{3a9}', VOWEL));
    }
    if !((0x370..=0x3ff).contains(&cp) || (0x1f00..=0x1fff).contains(&cp)) || !c.is_alphabetic() {
        return None;
    }
    let decomposed: String = DecomposingNormalizerBorrowed::new_nfd().normalize(&c.to_string()).into_owned();
    let mut chars = decomposed.chars();
    let base = chars.next()?;
    // U+0374 NUMERAL SIGN decomposes to U+02B9: not a Greek letter in ICU's tables.
    if !(0x370..=0x3ff).contains(&(base as u32)) {
        return None;
    }
    let mut flags = 0;
    for m in chars {
        match m as u32 {
            0x300 | 0x301 | 0x342 => flags |= ACCENT,
            0x308 => flags |= DIALYTIKA,
            0x345 => flags |= YPOGEGRAMMENI,
            _ => {}
        }
    }
    let mut up = base.to_uppercase();
    let upper = match (up.next(), up.next()) {
        (Some(u), None) => u,
        _ => base,
    };
    if matches!(upper as u32, 0x391 | 0x395 | 0x397 | 0x399 | 0x39f | 0x3a5 | 0x3a9) {
        flags |= VOWEL;
    }
    Some((upper, flags))
}

/// ICU `getDiacriticData`: the flags of a combining mark that follows a Greek
/// letter (the mark is swallowed and folded into the letter's flags).
fn diacritic_data(c: char) -> u32 {
    match c as u32 {
        0x300 | 0x301 | 0x342 | 0x302 | 0x303 | 0x311 => ACCENT,
        0x308 => COMBINING_DIALYTIKA,
        0x344 => COMBINING_DIALYTIKA | ACCENT,
        0x345 => YPOGEGRAMMENI,
        // 0x304, 0x306, 0x313, 0x314, 0x343: dropped, no flag.
        0x304 | 0x306 | 0x313 | 0x314 | 0x343 => 1,
        _ => 0,
    }
}

const AFTER_CASED: u32 = 1;
const AFTER_VOWEL_WITH_COMBINING_ACCENT: u32 = 2;
const AFTER_VOWEL_WITH_PRECOMPOSED_ACCENT: u32 = 4;

/// ICU `isFollowedByCasedLetter`: skips case-ignorable characters, then asks
/// whether the next one is cased (same "word boundary" test as Final_Sigma).
fn followed_by_cased_letter(rest: &[char]) -> bool {
    let cased = CodePointSetData::new::<Cased>();
    let ignorable = CodePointSetData::new::<CaseIgnorable>();
    for &c in rest {
        if ignorable.contains(c) {
            continue;
        }
        return cased.contains(c);
    }
    false
}

/// `s.toLocaleUpperCase("el")` as ICU does it.
pub(crate) fn greek_upper(s: &str) -> String {
    let src: Vec<char> = s.chars().collect();
    let cased = CodePointSetData::new::<Cased>();
    let ignorable = CodePointSetData::new::<CaseIgnorable>();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut state = 0u32;
    while i < src.len() {
        let c = src[i];
        let mut next = i + 1;
        let mut next_state = 0u32;
        if ignorable.contains(c) {
            next_state |= state & AFTER_CASED;
        } else if cased.contains(c) {
            next_state |= AFTER_CASED;
        }
        let Some((mut upper, mut data)) = letter_data(c) else {
            out.extend(c.to_uppercase());
            i = next;
            state = next_state;
            continue;
        };
        // A dialytika is added to an iota or upsilon after a vowel that lost
        // an accent and has no dialytika of its own. ICU decides this on the
        // letter's own (precomposed) data, before looking at following marks.
        if data & VOWEL != 0 && data & DIALYTIKA == 0 && matches!(upper as u32, 0x399 | 0x3a5) {
            if state & AFTER_VOWEL_WITH_PRECOMPOSED_ACCENT != 0 {
                data |= DIALYTIKA;
            } else if state & AFTER_VOWEL_WITH_COMBINING_ACCENT != 0 {
                data |= COMBINING_DIALYTIKA;
            }
        }
        let precomposed_accent = data & ACCENT != 0;
        let mut ypogegrammeni = u32::from(data & YPOGEGRAMMENI != 0);
        // Swallow the combining marks that follow the letter.
        while next < src.len() {
            let d = diacritic_data(src[next]);
            if d == 0 {
                break;
            }
            data |= d;
            if d & YPOGEGRAMMENI != 0 {
                ypogegrammeni += 1;
            }
            next += 1;
        }
        if data & VOWEL != 0 && data & ACCENT != 0 && data & DIALYTIKA == 0 {
            next_state |= if precomposed_accent {
                AFTER_VOWEL_WITH_PRECOMPOSED_ACCENT
            } else {
                AFTER_VOWEL_WITH_COMBINING_ACCENT
            };
        }
        let mut add_tonos = false;
        if upper as u32 == 0x397
            && data & ACCENT != 0
            && ypogegrammeni == 0
            && state & AFTER_CASED == 0
            && !followed_by_cased_letter(&src[next..])
        {
            // The disjunctive "or" keeps its tonos.
            if precomposed_accent {
                upper = '\u{389}';
            } else {
                add_tonos = true;
            }
        } else if data & DIALYTIKA != 0 {
            // Keep a vowel with dialytika in its precomposed form.
            if upper as u32 == 0x399 {
                upper = '\u{3aa}';
                data &= !EITHER_DIALYTIKA;
            } else if upper as u32 == 0x3a5 {
                upper = '\u{3ab}';
                data &= !EITHER_DIALYTIKA;
            }
        }
        out.push(upper);
        if data & EITHER_DIALYTIKA != 0 {
            out.push('\u{308}');
        }
        if add_tonos {
            out.push('\u{301}');
        }
        for _ in 0..ypogegrammeni {
            out.push('\u{399}');
        }
        i = next;
        state = next_state;
    }
    out
}
