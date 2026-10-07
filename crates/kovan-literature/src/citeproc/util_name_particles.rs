// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_name_particles.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file(s) named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/util_name_particles.js`: `CSL.ParticleList` (a data table
//! upstream never reads) and `CSL.parseParticles`, which splits a name's
//! `family` / `given` into non-dropping particles, dropping particles and a
//! suffix (`"Doe, Jr."`).
//!
//! A name is a JS object here a [`js::Obj`]: `family`, `given`, `suffix`,
//! `non-dropping-particle`, `dropping-particle`, `comma-suffix`,
//! `comma-dropping-particle`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::{CslResult, EngineError};

// DUP-CHECK: none (CSL.ParticleList lives only in util_name_particles.js)
/// `CSL.ParticleList`: `[particle, [[dropping_from,dropping_to] | None, [non_dropping_from, non_dropping_to] | None] ...]`, flattened as
/// `(particle, &[(drop, nondrop)])` where each range is `Some((from, to))`.
type Range = Option<(u8, u8)>;
const PARTICLE_LIST: [(&str, &[(Range, Range)]); 222] = [
    ("'s", &[(None, Some((0, 1)))]),
    ("'s-", &[(None, Some((0, 1)))]),
    ("'t", &[(None, Some((0, 1)))]),
    ("a", &[(None, Some((0, 1)))]),
    ("aan 't", &[(None, Some((0, 2)))]),
    ("aan de", &[(None, Some((0, 2)))]),
    ("aan den", &[(None, Some((0, 2)))]),
    ("aan der", &[(None, Some((0, 2)))]),
    ("aan het", &[(None, Some((0, 2)))]),
    ("aan t", &[(None, Some((0, 2)))]),
    ("aan", &[(None, Some((0, 1)))]),
    ("ad-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("adh-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("af", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("al", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("al-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("am de", &[(None, Some((0, 2)))]),
    ("am", &[(None, Some((0, 1)))]),
    ("an-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("ar-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("as-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("ash-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("at-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("ath-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("auf dem", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("auf den", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("auf der", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("auf ter", &[(None, Some((0, 2)))]),
    ("auf", &[(Some((0, 1)), None), (None, Some((0, 1)))]),
    ("aus 'm", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("aus dem", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("aus den", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("aus der", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("aus m", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("aus", &[(Some((0, 1)), None), (None, Some((0, 1)))]),
    ("aus'm", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("az-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aš-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aḍ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aḏ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aṣ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aṭ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aṯ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("aẓ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("ben", &[(None, Some((0, 1)))]),
    ("bij 't", &[(None, Some((0, 2)))]),
    ("bij de", &[(None, Some((0, 2)))]),
    ("bij den", &[(None, Some((0, 2)))]),
    ("bij het", &[(None, Some((0, 2)))]),
    ("bij t", &[(None, Some((0, 2)))]),
    ("bij", &[(None, Some((0, 1)))]),
    ("bin", &[(None, Some((0, 1)))]),
    ("boven d", &[(None, Some((0, 2)))]),
    ("boven d'", &[(None, Some((0, 2)))]),
    ("d", &[(None, Some((0, 1)))]),
    ("d'", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("da", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("dal", &[(None, Some((0, 1)))]),
    ("dal'", &[(None, Some((0, 1)))]),
    ("dall'", &[(None, Some((0, 1)))]),
    ("dalla", &[(None, Some((0, 1)))]),
    ("das", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("de die le", &[(None, Some((0, 3)))]),
    ("de die", &[(None, Some((0, 2)))]),
    ("de l", &[(None, Some((0, 2)))]),
    ("de l'", &[(None, Some((0, 2)))]),
    ("de la", &[(None, Some((0, 2))), (Some((0, 1)), Some((1, 2)))]),
    ("de las", &[(None, Some((0, 2))), (Some((0, 1)), Some((1, 2)))]),
    ("de le", &[(None, Some((0, 2)))]),
    ("de li", &[(None, Some((0, 2))), (Some((0, 2)), None)]),
    ("de van der", &[(None, Some((0, 3)))]),
    ("de", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("de'", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("deca", &[(None, Some((0, 1)))]),
    ("degli", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("dei", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("del", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("dela", &[(Some((0, 1)), None)]),
    ("dell'", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("della", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("delle", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("dello", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("den", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("der", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("des", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("di", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("die le", &[(None, Some((0, 2)))]),
    ("do", &[(None, Some((0, 1)))]),
    ("don", &[(None, Some((0, 1)))]),
    ("dos", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("du", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("ed-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("edh-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("el", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("el-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("en-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("er-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("es-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("esh-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("et-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eth-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("ez-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eš-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eḍ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eḏ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eṣ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eṭ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eṯ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("eẓ-", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("het", &[(None, Some((0, 1)))]),
    ("i", &[(None, Some((0, 1)))]),
    ("il", &[(Some((0, 1)), None)]),
    ("im", &[(None, Some((0, 1)))]),
    ("in 't", &[(None, Some((0, 2)))]),
    ("in de", &[(None, Some((0, 2)))]),
    ("in den", &[(None, Some((0, 2)))]),
    ("in der", &[(None, Some((0, 2))), (Some((0, 2)), None)]),
    ("in het", &[(None, Some((0, 2)))]),
    ("in t", &[(None, Some((0, 2)))]),
    ("in", &[(None, Some((0, 1)))]),
    ("l", &[(None, Some((0, 1)))]),
    ("l'", &[(None, Some((0, 1)))]),
    ("la", &[(None, Some((0, 1)))]),
    ("las", &[(None, Some((0, 1)))]),
    ("le", &[(None, Some((0, 1)))]),
    ("les", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("lo", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("los", &[(None, Some((0, 1)))]),
    ("lou", &[(None, Some((0, 1)))]),
    ("of", &[(None, Some((0, 1)))]),
    ("onder 't", &[(None, Some((0, 2)))]),
    ("onder de", &[(None, Some((0, 2)))]),
    ("onder den", &[(None, Some((0, 2)))]),
    ("onder het", &[(None, Some((0, 2)))]),
    ("onder t", &[(None, Some((0, 2)))]),
    ("onder", &[(None, Some((0, 1)))]),
    ("op 't", &[(None, Some((0, 2)))]),
    ("op de", &[(None, Some((0, 2))), (Some((0, 2)), None)]),
    ("op den", &[(None, Some((0, 2)))]),
    ("op der", &[(None, Some((0, 2)))]),
    ("op gen", &[(None, Some((0, 2)))]),
    ("op het", &[(None, Some((0, 2)))]),
    ("op t", &[(None, Some((0, 2)))]),
    ("op ten", &[(None, Some((0, 2)))]),
    ("op", &[(None, Some((0, 1)))]),
    ("over 't", &[(None, Some((0, 2)))]),
    ("over de", &[(None, Some((0, 2)))]),
    ("over den", &[(None, Some((0, 2)))]),
    ("over het", &[(None, Some((0, 2)))]),
    ("over t", &[(None, Some((0, 2)))]),
    ("over", &[(None, Some((0, 1)))]),
    ("s", &[(None, Some((0, 1)))]),
    ("s'", &[(None, Some((0, 1)))]),
    ("sen", &[(Some((0, 1)), None)]),
    ("t", &[(None, Some((0, 1)))]),
    ("te", &[(None, Some((0, 1)))]),
    ("ten", &[(None, Some((0, 1)))]),
    ("ter", &[(None, Some((0, 1)))]),
    ("tho", &[(None, Some((0, 1)))]),
    ("thoe", &[(None, Some((0, 1)))]),
    ("thor", &[(None, Some((0, 1)))]),
    ("to", &[(None, Some((0, 1)))]),
    ("toe", &[(None, Some((0, 1)))]),
    ("tot", &[(None, Some((0, 1)))]),
    ("uijt 't", &[(None, Some((0, 2)))]),
    ("uijt de", &[(None, Some((0, 2)))]),
    ("uijt den", &[(None, Some((0, 2)))]),
    ("uijt te de", &[(None, Some((0, 3)))]),
    ("uijt ten", &[(None, Some((0, 2)))]),
    ("uijt", &[(None, Some((0, 1)))]),
    ("uit 't", &[(None, Some((0, 2)))]),
    ("uit de", &[(None, Some((0, 2)))]),
    ("uit den", &[(None, Some((0, 2)))]),
    ("uit het", &[(None, Some((0, 2)))]),
    ("uit t", &[(None, Some((0, 2)))]),
    ("uit te de", &[(None, Some((0, 3)))]),
    ("uit ten", &[(None, Some((0, 2)))]),
    ("uit", &[(None, Some((0, 1)))]),
    ("unter", &[(None, Some((0, 1)))]),
    ("v", &[(None, Some((0, 1)))]),
    ("v.", &[(None, Some((0, 1)))]),
    ("v.d.", &[(None, Some((0, 1)))]),
    ("van 't", &[(None, Some((0, 2)))]),
    ("van de l", &[(None, Some((0, 3)))]),
    ("van de l'", &[(None, Some((0, 3)))]),
    ("van de", &[(None, Some((0, 2)))]),
    ("van de", &[(None, Some((0, 2)))]),
    ("van den", &[(None, Some((0, 2)))]),
    ("van der", &[(None, Some((0, 2)))]),
    ("van gen", &[(None, Some((0, 2)))]),
    ("van het", &[(None, Some((0, 2)))]),
    ("van la", &[(None, Some((0, 2)))]),
    ("van t", &[(None, Some((0, 2)))]),
    ("van ter", &[(None, Some((0, 2)))]),
    ("van van de", &[(None, Some((0, 3)))]),
    ("van", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("vander", &[(None, Some((0, 1)))]),
    ("vd", &[(None, Some((0, 1)))]),
    ("ver", &[(None, Some((0, 1)))]),
    ("vom und zum", &[(Some((0, 3)), None)]),
    ("vom", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("von 't", &[(None, Some((0, 2)))]),
    ("von dem", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("von den", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("von der", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("von t", &[(None, Some((0, 2)))]),
    ("von und zu", &[(Some((0, 3)), None), (None, Some((0, 3)))]),
    ("von zu", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("von", &[(Some((0, 1)), None), (None, Some((0, 1)))]),
    ("voor 't", &[(None, Some((0, 2)))]),
    ("voor de", &[(None, Some((0, 2)))]),
    ("voor den", &[(None, Some((0, 2)))]),
    ("voor in 't", &[(None, Some((0, 3)))]),
    ("voor in t", &[(None, Some((0, 3)))]),
    ("voor", &[(None, Some((0, 1)))]),
    ("vor der", &[(Some((0, 2)), None), (None, Some((0, 2)))]),
    ("vor", &[(Some((0, 1)), None), (None, Some((0, 1)))]),
    ("z", &[(Some((0, 1)), None)]),
    ("ze", &[(Some((0, 1)), None)]),
    ("zu", &[(Some((0, 1)), None), (None, Some((0, 1)))]),
    ("zum", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
    ("zur", &[(None, Some((0, 1))), (Some((0, 1)), None)]),
];

/// JS `.`: anything but `\n`, `\r`, U+2028, U+2029.
const DOT: &str = "[^\\n\\r\\u{2028}\\u{2029}]";

// DUP-CHECK: load.js CSL.PARTICLE_GIVEN_REGEXP
// /^([^ ]+(?:ʻ |’ | |\' ) *)(.+)$/
static PARTICLE_GIVEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "^([^ ]+(?:\u{02bb} |\u{2019} | |' ) *)({DOT}+)$"
    ))
    .unwrap_or_else(|e| panic!("static regex: {e}"))
});

// DUP-CHECK: load.js CSL.PARTICLE_FAMILY_REGEXP
// /^([^ ]+(?:\-|ʻ|’| |\') *)(.+)$/
static PARTICLE_FAMILY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "^([^ ]+(?:-|\u{02bb}|\u{2019}| |') *)({DOT}+)$"
    ))
    .unwrap_or_else(|e| panic!("static regex: {e}"))
});

/// `/^[-\'ʻ’\s]*(.).*$/`.
static FIRST_CHAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "^[-'\u{02bb}\u{2019}{ws}]*({DOT}){DOT}*$",
        ws = js::WS
    ))
    .unwrap_or_else(|e| panic!("static regex: {e}"))
});

/// `/(\s*,!*\s*)/`.
static SUFFIX_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("([{ws}]*,!*[{ws}]*)", ws = js::WS))
        .unwrap_or_else(|e| panic!("static regex: {e}"))
});

/// The TypeError JS raises for `value.<method>(...)` when `value` is not a
/// string: undefined / null have no properties; anything else has no such
/// method.
fn not_a_string(v: Option<&Value>, what: &str, method: &str) -> EngineError {
    EngineError::Csl(match v {
        None => format!("Cannot read properties of undefined (reading '{method}')"),
        Some(Value::Null) => format!("Cannot read properties of null (reading '{method}')"),
        Some(_) => format!("{what}.{method} is not a function"),
    })
}

/// JS `s.split("").reverse().join("")` on characters.
fn reverse(s: &str) -> String {
    s.chars().rev().collect()
}

/// `splitParticles(nameValue, firstNameFlag)` (the `caseOverride` argument is
/// never used upstream). Returns `(hasParticle, remaining name, particles)`.
fn split_particles(name_value: &str, first_name_flag: bool) -> (bool, String, Vec<String>) {
    let mut orig = name_value.to_string();
    let mut particle_list: Vec<String> = Vec::new();
    let mut has_particle = false;
    let (rex, mut name_value): (&Regex, String) = if first_name_flag {
        (&PARTICLE_GIVEN_RE, reverse(name_value))
    } else {
        (&PARTICLE_FAMILY_RE, name_value.to_string())
    };
    loop {
        let Some(caps) = rex.captures(&name_value) else {
            break;
        };
        let g1 = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let g2 = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
        let m1 = if first_name_flag { reverse(g1) } else { g1.to_string() };
        let first_char = FIRST_CHAR_RE.replace(&m1, "${1}").into_owned();
        has_particle = !first_char.is_empty() && first_char.to_uppercase() != first_char;
        if !has_particle {
            break;
        }
        let l = js::len(&m1) as i64;
        if first_name_flag {
            particle_list.push(js::slice(&orig, -l, None));
            orig = js::slice(&orig, 0, Some(-l));
        } else {
            particle_list.push(js::slice(&orig, 0, Some(l)));
            orig = js::slice(&orig, l, None);
        }
        name_value = g2;
    }
    if first_name_flag {
        name_value = reverse(&name_value);
        particle_list.reverse();
        for i in 1..particle_list.len() {
            if js::slice(&particle_list[i], 0, Some(1)) == " " {
                particle_list[i - 1].push(' ');
            }
        }
        for p in particle_list.iter_mut() {
            if js::slice(p, 0, Some(1)) == " " {
                *p = js::slice(p, 1, None);
            }
        }
        name_value = js::slice(&orig, 0, Some(js::len(&name_value) as i64));
    } else {
        name_value = js::slice(&orig, -(js::len(&name_value) as i64), None);
    }
    (has_particle, name_value, particle_list)
}

/// `trimLast(str)`: trim, but keep one trailing space after an apostrophe
/// that had one (`"d' "`).
fn trim_last(s: &str) -> String {
    let last_char = js::slice(s, -1, None);
    let mut s = js::trim(s).to_string();
    if last_char == " " {
        let l = js::slice(&s, -1, None);
        if l == "'" || l == "\u{2019}" {
            s.push(' ');
        }
    }
    s
}

/// `parseSuffix(nameObj)`: split `given` at the first comma into given and
/// suffix, or move a trailing `et al` into a dropping particle.
fn parse_suffix(name_obj: &mut Obj) -> CslResult<()> {
    if js::get_truthy(name_obj, "suffix") || !js::get_truthy(name_obj, "given") {
        return Ok(());
    }
    let given = match name_obj.get("given") {
        Some(Value::String(s)) => s.clone(),
        other => return Err(not_a_string(other, "nameObj.given", "match")),
    };
    let Some(m) = SUFFIX_COMMA_RE.captures(&given) else {
        return Ok(());
    };
    let m1 = m.get(1).map(|x| x.as_str()).unwrap_or("");
    let idx = js::index_of(&given, m1, 0);
    let l1 = js::len(m1) as i64;
    let possible_suffix = js::slice(&given, idx + l1, None);
    let possible_comma: String = js::slice(&given, idx, Some(idx + l1))
        .chars()
        .filter(|c| !(c.is_whitespace() || *c == '\u{feff}'))
        .collect();
    if possible_suffix.replace('.', "") == "et al" && !js::get_truthy(name_obj, "dropping-particle") {
        // This hack covers the case where "et al." is explicitly used in the
        // authorship information of the work.
        name_obj.insert("dropping-particle".into(), Value::String(possible_suffix));
        name_obj.insert("comma-dropping-particle".into(), Value::String(",".into()));
    } else {
        if js::len(&possible_comma) == 2 {
            name_obj.insert("comma-suffix".into(), Value::Bool(true));
        }
        name_obj.insert("suffix".into(), Value::String(possible_suffix));
    }
    name_obj.insert("given".into(), Value::String(js::slice(&given, 0, Some(idx))));
    Ok(())
}

/// The string value of `name[key]` for `splitParticles`, which calls
/// `method` (`match` for the family, `split` for the given name) on it; a
/// missing or non-string value is a TypeError upstream.
fn string_field(name_obj: &Obj, key: &str, method: &str) -> CslResult<String> {
    match name_obj.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        other => Err(not_a_string(other, "nameValue", method)),
    }
}

/// `CSL.parseParticles(nameObj)`: extract non-dropping particles from
/// `family`, split a suffix off `given`, extract dropping particles from
/// `given`; the name object is updated in place.
///
/// Errors (as JS would throw) when `family` or `given` is not a string.
pub fn parse_particles(name_obj: &mut Obj) -> CslResult<()> {
    let family = string_field(name_obj, "family", "match")?;
    let (_, last_name_value, last_particle_list) = split_particles(&family, false);
    name_obj.insert("family".into(), Value::String(last_name_value));
    let non_dropping = trim_last(&last_particle_list.join(""));
    if !non_dropping.is_empty() {
        name_obj.insert("non-dropping-particle".into(), Value::String(non_dropping));
    }
    // Split off suffix first of all
    parse_suffix(name_obj)?;
    // Extract and set dropping particle(s) from given name field
    let given = string_field(name_obj, "given", "split")?;
    let (_, first_name_value, first_particle_list) = split_particles(&given, true);
    name_obj.insert("given".into(), Value::String(first_name_value));
    let dropping = js::trim(&first_particle_list.join("")).to_string();
    if !dropping.is_empty() {
        name_obj.insert("dropping-particle".into(), Value::String(dropping));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63: reference
    //! `tests/data/csl/units/names.json` (generator
    //! `scripts/csl-units/names.cjs`): `CSL.parseParticles` on ~3,000 name
    //! objects (every name in the CSL test suite's fixtures plus 50 family x
    //! 37 given-name combinations built to exercise particles, suffixes,
    //! `et al`, quotes, apostrophes, CJK and Vietnamese names, and name
    //! objects with missing or non-string fields). Pass criterion: the same
    //! name object (or the same TypeError text) as citeproc-js.
    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/names.json");

    #[test]
    fn parse_particles_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        for c in r["cases"].as_array().expect("cases") {
            let Value::Object(name) = &c["name"] else { continue };
            let mut copy = name.clone();
            let res = parse_particles(&mut copy);
            n += 1;
            match (res, c.get("particles_error")) {
                (Ok(()), None) => assert_eq!(Value::Object(copy), c["particles"], "name {}", c["name"]),
                (Err(e), Some(w)) => assert_eq!(
                    Some(match &e {
                        EngineError::Csl(m) => m.as_str(),
                        _ => "",
                    }),
                    w.as_str(),
                    "name {}",
                    c["name"]
                ),
                (g, w) => panic!("name {}: {g:?} vs {w:?}", c["name"]),
            }
        }
        assert!(n > 3000);
    }

    #[test]
    fn particle_list_is_upstreams() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let want = r["particle_list"].as_array().expect("list");
        assert_eq!(PARTICLE_LIST.len(), want.len());
        let rng = |x: &Option<(u8, u8)>| match x {
            None => Value::Null,
            Some((a, b)) => serde_json::json!([a, b]),
        };
        for (have, w) in PARTICLE_LIST.iter().zip(want) {
            assert_eq!(Value::String(have.0.to_string()), w[0]);
            let alts: Vec<Value> = have
                .1
                .iter()
                .map(|(d, n)| serde_json::json!([rng(d), rng(n)]))
                .collect();
            assert_eq!(Value::Array(alts), w[1], "particle {}", have.0);
        }
    }
}
