// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6; utilities.js is byte-identical in the translation-server's
//   1dd38e27edf8): utilities.js `cleanAuthor` :227-290, `trim` :297-305,
//   `trimInternal` :310-317, `cleanDOI` :481-523, `text2html` :638-660,
//   `htmlSpecialChars` :668-693, `lpad` :975-981, `removeDiacritics`
//   :1153-1169 with `_diacriticsRemovalMap` :1171-1263,
//   `getCreatorsForType` :1291-1300, `fieldIsValidForType` :1308-1310;
//   utilities_item.js `itemTypeExists` :41-49.
// Copyright (c) 2009 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA; Corporation for Digital Scholarship.
//   `_diacriticsRemovalMap` is from http://lehelk.com/2011/05/06/script-to-remove-diacritics/
//   as carried in Zotero.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `Zotero.Utilities` (`ZU`) as the four ported translators use it.
//!
//! Dates (`strToDate`, `strToISO`, `formatDate`) are kovan-common's
//! [`kovan_common::zotero::date`]; `unescapeHTML` is [`super::html`].

use super::js;
use kovan_common::zotero::schema::is_valid_for_type;
use kovan_common::zotero::schema_generated::{Field, ItemType};
use regex::Regex;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `Zotero.Utilities.capitalizeName` (utilities.js:199-216; #756): each
/// space-separated part that is all upper or all lower case is lower-cased
/// and every letter at its start or after a non-letter upper-cased
/// (`XRegExp('(^|[^\\pL])\\pL', 'g')`, the whole match upper-cased). The
/// same as the private copy in `translators::crossref_unixref_xml`.
pub fn capitalize_name(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(&R, || r"(^|[^\p{L}])\p{L}".to_owned());
    s.split(' ')
        .map(|part| {
            if part.to_uppercase() == part || part.to_lowercase() == part {
                r.replace_all(&part.to_lowercase(), |c: &regex::Captures| c[0].to_uppercase())
                    .into_owned()
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Zotero.Utilities.trim` (:297-305).
pub fn trim(s: &str) -> String {
    js::trim(s).to_owned()
}

/// `Zotero.Utilities.trimInternal` (:310-317): every run of
/// `[\xA0\r\n\s]` becomes one space, then trim.
pub fn trim_internal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if js::is_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    js::trim(&out).to_owned()
}

/// `Zotero.Utilities.lpad` (:975-981) for a string argument (an empty one
/// is treated as `''`, as `string ? string + '' : ''` does).
pub fn lpad(s: &str, pad: &str, length: usize) -> String {
    let mut out = s.to_owned();
    while js::len(&out) < length {
        out = format!("{pad}{out}");
    }
    out
}

/// The result of [`clean_author`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedAuthor {
    /// `firstName` (`None` is `undefined`).
    pub first_name: Option<String>,
    /// `lastName`.
    pub last_name: String,
    /// `creatorType`, as passed in.
    pub creator_type: String,
}

/// `"a b".split(/(?:[\s\.]+|(?=-))/)` with JavaScript's split algorithm (an
/// empty match at the position the previous piece ended is skipped).
fn split_names(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let (mut p, mut q) = (0usize, 0usize);
    while q < c.len() {
        // Try the alternatives at q, in order.
        let e = if js::is_space(c[q]) || c[q] == '.' {
            let mut e = q;
            while e < c.len() && (js::is_space(c[e]) || c[e] == '.') {
                e += 1;
            }
            Some(e)
        } else if c[q] == '-' {
            Some(q)
        } else {
            None
        };
        match e {
            Some(e) if e != p => {
                out.push(c[p..q].iter().collect());
                p = e;
                q = p;
            }
            _ => q += 1,
        }
    }
    out.push(c[p..].iter().collect());
    out
}

/// `Zotero.Utilities.cleanAuthor(author, type, useComma)` (:227-290).
pub fn clean_author(author: &str, creator_type: &str, use_comma: bool) -> CleanedAuthor {
    static LEAD: OnceLock<Regex> = OnceLock::new();
    static TRAIL: OnceLock<Regex> = OnceLock::new();
    static SPACE: OnceLock<Regex> = OnceLock::new();
    static PERIOD: OnceLock<Regex> = OnceLock::new();
    static COMMA: OnceLock<Regex> = OnceLock::new();
    static LETTER: OnceLock<Regex> = OnceLock::new();
    static ALL_CAPS: OnceLock<Regex> = OnceLock::new();
    static INITIAL: OnceLock<Regex> = OnceLock::new();
    static FIRST_LEAD: OnceLock<Regex> = OnceLock::new();
    static FIRST_TRAIL: OnceLock<Regex> = OnceLock::new();
    static DASH: OnceLock<Regex> = OnceLock::new();
    let ws = js::WS;
    // In a JS character class, \s inside [...] is the same set; spell it out.
    let ws_inner = &ws[1..ws.len() - 1];

    let author = re(&LEAD, || format!(r"^[{ws_inner}\x{{A0}}.,/\[\]:]+")).replace(author, "");
    let author = re(&TRAIL, || format!(r"[{ws_inner}\x{{A0}}.,/\[\]:]+$")).replace(&author, "");
    let author = re(&SPACE, || format!(r"[{ws_inner}\x{{A0}}]+")).replace(&author, " ");
    let mut author = author.into_owned();

    let (mut first_name, last_name): (Option<String>, String);
    if use_comma {
        // Add spaces between periods (first occurrence only).
        author = re(&PERIOD, || r"\.([^ ])".to_owned())
            .replace(&author, ". $1")
            .into_owned();
        let split: Vec<&str> = re(&COMMA, || "[,\u{060C}] ?".to_owned())
            .split(&author)
            .collect();
        if split.len() > 1 {
            last_name = split[0].to_owned();
            first_name = Some(split[1].to_owned());
        } else {
            last_name = author.clone();
            first_name = None;
        }
    } else {
        let chars: Vec<char> = author.chars().collect();
        let letter = re(&LETTER, || r"\p{L}".to_owned());
        let mut space_index: isize = chars.len() as isize;
        loop {
            // author.lastIndexOf(" ", spaceIndex - 1): a negative fromIndex
            // searches position 0 only.
            let from = (space_index - 1).max(0) as usize;
            space_index = chars[..chars.len().min(from + 1)]
                .iter()
                .rposition(|&c| c == ' ')
                .map_or(-1, |i| i as isize);
            let ln: String = chars[(space_index + 1) as usize..].iter().collect();
            let fnm: String = chars[..space_index.max(0) as usize].iter().collect();
            // `lastName[0]` is undefined for an empty last name, and
            // XRegExp('\\pL').test(undefined) tests the string "undefined".
            let first_is_letter = match ln.chars().next() {
                Some(c) => letter.is_match(&c.to_string()),
                None => true,
            };
            if first_is_letter || space_index <= 0 {
                last_name = ln;
                first_name = Some(fnm);
                break;
            }
        }
    }

    let all_caps = re(&ALL_CAPS, || r"^[A-Z\x{0400}-\x{042F}]+$".to_owned());
    let initial = re(&INITIAL, || r"^-?[A-Z\x{0400}-\x{042F}]$".to_owned());
    if let Some(f) = first_name.as_deref() {
        let flen = js::len(f);
        if !f.is_empty()
            && all_caps.is_match(f)
            && flen < 4
            && (flen == 1 || last_name.to_uppercase() != last_name)
        {
            // First name is probably initials.
            let mut n = String::new();
            for c in f.chars() {
                n.push(' ');
                n.push(c);
                n.push('.');
            }
            first_name = Some(n[1..].to_owned());
        }
    }

    if let Some(f) = first_name.as_deref().filter(|f| !f.is_empty()) {
        let f = re(&FIRST_LEAD, || format!(r"^[{ws_inner}.]+")).replace(f, "");
        let f = re(&FIRST_TRAIL, || format!(r"[{ws_inner},]+$")).replace(&f, "");
        let f = re(&DASH, || {
            format!(r"[{ws_inner}]*([\x{{2D}}\x{{AD}}\x{{2010}}-\x{{2015}}\x{{2212}}\x{{2E3A}}\x{{2E3B}}])[{ws_inner}]*")
        })
        .replace(&f, "-");
        let mut n = String::new();
        for name in split_names(&f) {
            n.push_str(&name);
            if initial.is_match(&name) {
                n.push('.');
            }
            n.push(' ');
        }
        first_name = Some(js::trim(&n.replace(" -", "-")).to_owned());
    }

    CleanedAuthor {
        first_name,
        last_name,
        creator_type: creator_type.to_owned(),
    }
}

/// JavaScript `decodeURIComponent`, or `None` where it throws `URIError`
/// (a `%` not followed by two hex digits, or bytes that are not UTF-8).
pub fn decode_uri_component(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = b.get(i + 1..i + 3)?;
            if !h.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            let v = u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()?;
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `Zotero.Utilities.cleanDOI` (:481-523): the DOI in `x`, or `None`
/// (upstream `null`).
pub fn clean_doi(x: &str) -> Option<String> {
    static HTTP: OnceLock<Regex> = OnceLock::new();
    static DOI: OnceLock<Regex> = OnceLock::new();
    let mut x = x.to_owned();
    if re(&HTTP, || "^https?:".to_owned()).is_match(&x) {
        if let Some(d) = decode_uri_component(&x) {
            x = d;
        }
    }
    if let Some(open) = x.find("%3C") {
        if x.find("%3E").is_some_and(|close| open < close) {
            x = x.replace("%3C", "<").replace("%3E", ">");
        }
    }
    let ws_inner = &js::WS[1..js::WS.len() - 1];
    let m = re(&DOI, || {
        format!(r"10(?:\.[0-9]{{4,}})?/[^{ws_inner}]*[^{ws_inner}.,]")
    })
    .find(&x)?;
    let mut result = m.as_str().to_owned();
    let last = result.chars().last();
    if let Some(tb) = last.filter(|c| [']', ')', '}'].contains(c)) {
        let before = &x[..m.start()];
        let ob = match tb {
            ']' => '[',
            ')' => '(',
            _ => '{',
        };
        let idx = |c: char| before.rfind(c).map_or(-1, |i| i as isize);
        if idx(ob) > idx(tb) {
            result.pop();
        }
    }
    Some(result)
}

/// `Zotero.Utilities.htmlSpecialChars` (:668-693).
pub fn html_special_chars(s: &str) -> String {
    static Z: OnceLock<Regex> = OnceLock::new();
    if s.is_empty() {
        return String::new();
    }
    let s = s
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    re(&Z, || "&lt;ZOTERO([^/]+)/&gt;".to_owned())
        .replace_all(&s, |c: &regex::Captures| match &c[1] {
            "BREAK" => "<br/>".to_owned(),
            "HELLIP" => "&#8230;".to_owned(),
            other => other.to_owned(),
        })
        .into_owned()
}

/// `Zotero.Utilities.text2html(str, singleNewlineIsParagraph)` (:638-660).
pub fn text2html(s: &str, single_newline_is_paragraph: bool) -> String {
    static EMPTY_P: OnceLock<Regex> = OnceLock::new();
    let s = html_special_chars(s);
    let s = if single_newline_is_paragraph {
        format!(
            "<p>{}</p>",
            s.replace('\n', "</p><p>").replace("  ", "&nbsp; ")
        )
    } else {
        format!(
            "<p>{}</p>",
            s.replace("\n\n", "</p><p>")
                .replace('\n', "<br/>")
                .replace("  ", "&nbsp; ")
        )
    };
    re(&EMPTY_P, || format!(r"<p>{}*</p>", js::WS))
        .replace_all(&s, "<p>&nbsp;</p>")
        .into_owned()
}

/// `_diacriticsRemovalMap.lowercase` (:1218-1262): base and letters.
const DIACRITICS_LOWER: &[(&str, &str)] = &[
    ("a", "\u{0061}\u{24D0}\u{FF41}\u{1E9A}\u{00E0}\u{00E1}\u{00E2}\u{1EA7}\u{1EA5}\u{1EAB}\u{1EA9}\u{00E3}\u{0101}\u{0103}\u{1EB1}\u{1EAF}\u{1EB5}\u{1EB3}\u{0227}\u{01E1}\u{00E4}\u{01DF}\u{1EA3}\u{00E5}\u{01FB}\u{01CE}\u{0201}\u{0203}\u{1EA1}\u{1EAD}\u{1EB7}\u{1E01}\u{0105}\u{2C65}\u{0250}"),
    ("aa", "\u{A733}"),
    ("ae", "\u{00E6}\u{01FD}\u{01E3}"),
    ("ao", "\u{A735}"),
    ("au", "\u{A737}"),
    ("av", "\u{A739}\u{A73B}"),
    ("ay", "\u{A73D}"),
    ("b", "\u{0062}\u{24D1}\u{FF42}\u{1E03}\u{1E05}\u{1E07}\u{0180}\u{0183}\u{0253}"),
    ("c", "\u{0063}\u{24D2}\u{FF43}\u{0107}\u{0109}\u{010B}\u{010D}\u{00E7}\u{1E09}\u{0188}\u{023C}\u{A73F}\u{2184}"),
    ("d", "\u{0064}\u{24D3}\u{FF44}\u{1E0B}\u{010F}\u{1E0D}\u{1E11}\u{1E13}\u{1E0F}\u{0111}\u{018C}\u{0256}\u{0257}\u{A77A}"),
    ("dz", "\u{01F3}\u{01C6}"),
    ("e", "\u{0065}\u{24D4}\u{FF45}\u{00E8}\u{00E9}\u{00EA}\u{1EC1}\u{1EBF}\u{1EC5}\u{1EC3}\u{1EBD}\u{0113}\u{1E15}\u{1E17}\u{0115}\u{0117}\u{00EB}\u{1EBB}\u{011B}\u{0205}\u{0207}\u{1EB9}\u{1EC7}\u{0229}\u{1E1D}\u{0119}\u{1E19}\u{1E1B}\u{0247}\u{025B}\u{01DD}"),
    ("f", "\u{0066}\u{24D5}\u{FF46}\u{1E1F}\u{0192}\u{A77C}"),
    ("g", "\u{0067}\u{24D6}\u{FF47}\u{01F5}\u{011D}\u{1E21}\u{011F}\u{0121}\u{01E7}\u{0123}\u{01E5}\u{0260}\u{A7A1}\u{1D79}\u{A77F}"),
    ("h", "\u{0068}\u{24D7}\u{FF48}\u{0125}\u{1E23}\u{1E27}\u{021F}\u{1E25}\u{1E29}\u{1E2B}\u{1E96}\u{0127}\u{2C68}\u{2C76}\u{0265}"),
    ("hv", "\u{0195}"),
    ("i", "\u{0069}\u{24D8}\u{FF49}\u{00EC}\u{00ED}\u{00EE}\u{0129}\u{012B}\u{012D}\u{00EF}\u{1E2F}\u{1EC9}\u{01D0}\u{0209}\u{020B}\u{1ECB}\u{012F}\u{1E2D}\u{0268}\u{0131}"),
    ("j", "\u{006A}\u{24D9}\u{FF4A}\u{0135}\u{01F0}\u{0249}"),
    ("k", "\u{006B}\u{24DA}\u{FF4B}\u{1E31}\u{01E9}\u{1E33}\u{0137}\u{1E35}\u{0199}\u{2C6A}\u{A741}\u{A743}\u{A745}\u{A7A3}"),
    ("l", "\u{006C}\u{24DB}\u{FF4C}\u{0140}\u{013A}\u{013E}\u{1E37}\u{1E39}\u{013C}\u{1E3D}\u{1E3B}\u{017F}\u{0142}\u{019A}\u{026B}\u{2C61}\u{A749}\u{A781}\u{A747}"),
    ("lj", "\u{01C9}"),
    ("m", "\u{006D}\u{24DC}\u{FF4D}\u{1E3F}\u{1E41}\u{1E43}\u{0271}\u{026F}"),
    ("n", "\u{006E}\u{24DD}\u{FF4E}\u{01F9}\u{0144}\u{00F1}\u{1E45}\u{0148}\u{1E47}\u{0146}\u{1E4B}\u{1E49}\u{019E}\u{0272}\u{0149}\u{A791}\u{A7A5}"),
    ("nj", "\u{01CC}"),
    ("o", "\u{006F}\u{24DE}\u{FF4F}\u{00F2}\u{00F3}\u{00F4}\u{1ED3}\u{1ED1}\u{1ED7}\u{1ED5}\u{00F5}\u{1E4D}\u{022D}\u{1E4F}\u{014D}\u{1E51}\u{1E53}\u{014F}\u{022F}\u{0231}\u{00F6}\u{022B}\u{1ECF}\u{0151}\u{01D2}\u{020D}\u{020F}\u{01A1}\u{1EDD}\u{1EDB}\u{1EE1}\u{1EDF}\u{1EE3}\u{1ECD}\u{1ED9}\u{01EB}\u{01ED}\u{00F8}\u{01FF}\u{0254}\u{A74B}\u{A74D}\u{0275}"),
    ("oe", "\u{0153}"),
    ("oi", "\u{01A3}"),
    ("ou", "\u{0223}"),
    ("oo", "\u{A74F}"),
    ("p", "\u{0070}\u{24DF}\u{FF50}\u{1E55}\u{1E57}\u{01A5}\u{1D7D}\u{A751}\u{A753}\u{A755}"),
    ("q", "\u{0071}\u{24E0}\u{FF51}\u{024B}\u{A757}\u{A759}"),
    ("r", "\u{0072}\u{24E1}\u{FF52}\u{0155}\u{1E59}\u{0159}\u{0211}\u{0213}\u{1E5B}\u{1E5D}\u{0157}\u{1E5F}\u{024D}\u{027D}\u{A75B}\u{A7A7}\u{A783}"),
    ("s", "\u{0073}\u{24E2}\u{FF53}\u{00DF}\u{015B}\u{1E65}\u{015D}\u{1E61}\u{0161}\u{1E67}\u{1E63}\u{1E69}\u{0219}\u{015F}\u{023F}\u{A7A9}\u{A785}\u{1E9B}"),
    ("t", "\u{0074}\u{24E3}\u{FF54}\u{1E6B}\u{1E97}\u{0165}\u{1E6D}\u{021B}\u{0163}\u{1E71}\u{1E6F}\u{0167}\u{01AD}\u{0288}\u{2C66}\u{A787}"),
    ("tz", "\u{A729}"),
    ("u", "\u{0075}\u{24E4}\u{FF55}\u{00F9}\u{00FA}\u{00FB}\u{0169}\u{1E79}\u{016B}\u{1E7B}\u{016D}\u{00FC}\u{01DC}\u{01D8}\u{01D6}\u{01DA}\u{1EE7}\u{016F}\u{0171}\u{01D4}\u{0215}\u{0217}\u{01B0}\u{1EEB}\u{1EE9}\u{1EEF}\u{1EED}\u{1EF1}\u{1EE5}\u{1E73}\u{0173}\u{1E77}\u{1E75}\u{0289}"),
    ("v", "\u{0076}\u{24E5}\u{FF56}\u{1E7D}\u{1E7F}\u{028B}\u{A75F}\u{028C}"),
    ("vy", "\u{A761}"),
    ("w", "\u{0077}\u{24E6}\u{FF57}\u{1E81}\u{1E83}\u{0175}\u{1E87}\u{1E85}\u{1E98}\u{1E89}\u{2C73}"),
    ("x", "\u{0078}\u{24E7}\u{FF58}\u{1E8B}\u{1E8D}"),
    ("y", "\u{0079}\u{24E8}\u{FF59}\u{1EF3}\u{00FD}\u{0177}\u{1EF9}\u{0233}\u{1E8F}\u{00FF}\u{1EF7}\u{1E99}\u{1EF5}\u{01B4}\u{024F}\u{1EFF}"),
    ("z", "\u{007A}\u{24E9}\u{FF5A}\u{017A}\u{1E91}\u{017C}\u{017E}\u{1E93}\u{1E95}\u{01B6}\u{0225}\u{0240}\u{2C6C}\u{A763}"),
];

/// `_diacriticsRemovalMap.uppercase` (:1172-1216).
const DIACRITICS_UPPER: &[(&str, &str)] = &[
    ("A", "\u{0041}\u{24B6}\u{FF21}\u{00C0}\u{00C1}\u{00C2}\u{1EA6}\u{1EA4}\u{1EAA}\u{1EA8}\u{00C3}\u{0100}\u{0102}\u{1EB0}\u{1EAE}\u{1EB4}\u{1EB2}\u{0226}\u{01E0}\u{00C4}\u{01DE}\u{1EA2}\u{00C5}\u{01FA}\u{01CD}\u{0200}\u{0202}\u{1EA0}\u{1EAC}\u{1EB6}\u{1E00}\u{0104}\u{023A}\u{2C6F}"),
    ("AA", "\u{A732}"),
    ("AE", "\u{00C6}\u{01FC}\u{01E2}"),
    ("AO", "\u{A734}"),
    ("AU", "\u{A736}"),
    ("AV", "\u{A738}\u{A73A}"),
    ("AY", "\u{A73C}"),
    ("B", "\u{0042}\u{24B7}\u{FF22}\u{1E02}\u{1E04}\u{1E06}\u{0243}\u{0182}\u{0181}"),
    ("C", "\u{0043}\u{24B8}\u{FF23}\u{0106}\u{0108}\u{010A}\u{010C}\u{00C7}\u{1E08}\u{0187}\u{023B}\u{A73E}"),
    ("D", "\u{0044}\u{24B9}\u{FF24}\u{1E0A}\u{010E}\u{1E0C}\u{1E10}\u{1E12}\u{1E0E}\u{0110}\u{018B}\u{018A}\u{0189}\u{A779}"),
    ("DZ", "\u{01F1}\u{01C4}"),
    ("Dz", "\u{01F2}\u{01C5}"),
    ("E", "\u{0045}\u{24BA}\u{FF25}\u{00C8}\u{00C9}\u{00CA}\u{1EC0}\u{1EBE}\u{1EC4}\u{1EC2}\u{1EBC}\u{0112}\u{1E14}\u{1E16}\u{0114}\u{0116}\u{00CB}\u{1EBA}\u{011A}\u{0204}\u{0206}\u{1EB8}\u{1EC6}\u{0228}\u{1E1C}\u{0118}\u{1E18}\u{1E1A}\u{0190}\u{018E}"),
    ("F", "\u{0046}\u{24BB}\u{FF26}\u{1E1E}\u{0191}\u{A77B}"),
    ("G", "\u{0047}\u{24BC}\u{FF27}\u{01F4}\u{011C}\u{1E20}\u{011E}\u{0120}\u{01E6}\u{0122}\u{01E4}\u{0193}\u{A7A0}\u{A77D}\u{A77E}"),
    ("H", "\u{0048}\u{24BD}\u{FF28}\u{0124}\u{1E22}\u{1E26}\u{021E}\u{1E24}\u{1E28}\u{1E2A}\u{0126}\u{2C67}\u{2C75}\u{A78D}"),
    ("I", "\u{0049}\u{24BE}\u{FF29}\u{00CC}\u{00CD}\u{00CE}\u{0128}\u{012A}\u{012C}\u{0130}\u{00CF}\u{1E2E}\u{1EC8}\u{01CF}\u{0208}\u{020A}\u{1ECA}\u{012E}\u{1E2C}\u{0197}"),
    ("J", "\u{004A}\u{24BF}\u{FF2A}\u{0134}\u{0248}"),
    ("K", "\u{004B}\u{24C0}\u{FF2B}\u{1E30}\u{01E8}\u{1E32}\u{0136}\u{1E34}\u{0198}\u{2C69}\u{A740}\u{A742}\u{A744}\u{A7A2}"),
    ("L", "\u{004C}\u{24C1}\u{FF2C}\u{013F}\u{0139}\u{013D}\u{1E36}\u{1E38}\u{013B}\u{1E3C}\u{1E3A}\u{0141}\u{023D}\u{2C62}\u{2C60}\u{A748}\u{A746}\u{A780}"),
    ("LJ", "\u{01C7}"),
    ("Lj", "\u{01C8}"),
    ("M", "\u{004D}\u{24C2}\u{FF2D}\u{1E3E}\u{1E40}\u{1E42}\u{2C6E}\u{019C}"),
    ("N", "\u{004E}\u{24C3}\u{FF2E}\u{01F8}\u{0143}\u{00D1}\u{1E44}\u{0147}\u{1E46}\u{0145}\u{1E4A}\u{1E48}\u{0220}\u{019D}\u{A790}\u{A7A4}"),
    ("NJ", "\u{01CA}"),
    ("Nj", "\u{01CB}"),
    ("O", "\u{004F}\u{24C4}\u{FF2F}\u{00D2}\u{00D3}\u{00D4}\u{1ED2}\u{1ED0}\u{1ED6}\u{1ED4}\u{00D5}\u{1E4C}\u{022C}\u{1E4E}\u{014C}\u{1E50}\u{1E52}\u{014E}\u{022E}\u{0230}\u{00D6}\u{022A}\u{1ECE}\u{0150}\u{01D1}\u{020C}\u{020E}\u{01A0}\u{1EDC}\u{1EDA}\u{1EE0}\u{1EDE}\u{1EE2}\u{1ECC}\u{1ED8}\u{01EA}\u{01EC}\u{00D8}\u{01FE}\u{0186}\u{019F}\u{A74A}\u{A74C}"),
    ("OE", "\u{0152}"),
    ("OI", "\u{01A2}"),
    ("OO", "\u{A74E}"),
    ("OU", "\u{0222}"),
    ("P", "\u{0050}\u{24C5}\u{FF30}\u{1E54}\u{1E56}\u{01A4}\u{2C63}\u{A750}\u{A752}\u{A754}"),
    ("Q", "\u{0051}\u{24C6}\u{FF31}\u{A756}\u{A758}\u{024A}"),
    ("R", "\u{0052}\u{24C7}\u{FF32}\u{0154}\u{1E58}\u{0158}\u{0210}\u{0212}\u{1E5A}\u{1E5C}\u{0156}\u{1E5E}\u{024C}\u{2C64}\u{A75A}\u{A7A6}\u{A782}"),
    ("S", "\u{0053}\u{24C8}\u{FF33}\u{1E9E}\u{015A}\u{1E64}\u{015C}\u{1E60}\u{0160}\u{1E66}\u{1E62}\u{1E68}\u{0218}\u{015E}\u{2C7E}\u{A7A8}\u{A784}"),
    ("T", "\u{0054}\u{24C9}\u{FF34}\u{1E6A}\u{0164}\u{1E6C}\u{021A}\u{0162}\u{1E70}\u{1E6E}\u{0166}\u{01AC}\u{01AE}\u{023E}\u{A786}"),
    ("TZ", "\u{A728}"),
    ("U", "\u{0055}\u{24CA}\u{FF35}\u{00D9}\u{00DA}\u{00DB}\u{0168}\u{1E78}\u{016A}\u{1E7A}\u{016C}\u{00DC}\u{01DB}\u{01D7}\u{01D5}\u{01D9}\u{1EE6}\u{016E}\u{0170}\u{01D3}\u{0214}\u{0216}\u{01AF}\u{1EEA}\u{1EE8}\u{1EEE}\u{1EEC}\u{1EF0}\u{1EE4}\u{1E72}\u{0172}\u{1E76}\u{1E74}\u{0244}"),
    ("V", "\u{0056}\u{24CB}\u{FF36}\u{1E7C}\u{1E7E}\u{01B2}\u{A75E}\u{0245}"),
    ("VY", "\u{A760}"),
    ("W", "\u{0057}\u{24CC}\u{FF37}\u{1E80}\u{1E82}\u{0174}\u{1E86}\u{1E84}\u{1E88}\u{2C72}"),
    ("X", "\u{0058}\u{24CD}\u{FF38}\u{1E8A}\u{1E8C}"),
    ("Y", "\u{0059}\u{24CE}\u{FF39}\u{1EF2}\u{00DD}\u{0176}\u{1EF8}\u{0232}\u{1E8E}\u{0178}\u{1EF6}\u{1EF4}\u{01B3}\u{024E}\u{1EFE}"),
    ("Z", "\u{005A}\u{24CF}\u{FF3A}\u{0179}\u{1E90}\u{017B}\u{017D}\u{1E92}\u{1E94}\u{01B5}\u{0224}\u{2C7F}\u{2C6B}\u{A762}"),
];

/// `Zotero.Utilities.removeDiacritics(str, lowercaseOnly)` (:1153-1169).
pub fn remove_diacritics(s: &str, lowercase_only: bool) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return s.to_owned();
    }
    let map = |s: String, table: &[(&str, &str)]| -> String {
        let mut s = s;
        for (base, letters) in table {
            if s.chars().any(|c| letters.contains(c)) {
                s = s
                    .chars()
                    .map(|c| {
                        if letters.contains(c) {
                            (*base).to_owned()
                        } else {
                            c.to_string()
                        }
                    })
                    .collect();
            }
        }
        s
    };
    let s = map(s.to_owned(), DIACRITICS_LOWER);
    if lowercase_only {
        s
    } else {
        map(s, DIACRITICS_UPPER)
    }
}

/// `Zotero.Utilities.getCreatorsForType` (:1291-1300): the creator types
/// valid for an item type, primary first. The order is schema.json's; the
/// translation-server's type snapshot (`zoteroTypeSchemaData.js`) orders
/// some secondary types differently, but the primary (first) type agrees for
/// every item type (checked 2026-10-07).
pub fn get_creators_for_type(item_type: &str) -> Vec<&'static str> {
    if item_type == "attachment" || item_type == "note" {
        return Vec::new();
    }
    ItemType::from_name(item_type)
        .map(|t| {
            t.schema()
                .creator_types
                .iter()
                .map(|c| c.as_str())
                .collect()
        })
        .unwrap_or_default()
}

/// `Zotero.Date.strToISO(str)` (date.js:600-614, the same in utilities
/// 1dd38e27edf8 and 4051881d59c6) on kovan-common's `strToDate`, with
/// upstream's truthiness: `if (date.year)` is true for any non-empty year
/// string, including `"0"` (what `strToDate("0000")` gives), so
/// `strToISO("0000")` is `"0000"` (checked by running upstream's date.js,
/// 2026-10-07). ~~kovan-common's
/// [`str_to_iso`](kovan_common::zotero::date::str_to_iso) returns `None` for
/// a `"0"` year; this one is used by the translators.~~ **CORRECTED
/// 2026-10-07:** kovan-common's `str_to_iso` was fixed the same day and now
/// agrees on year 0; this copy is kept because it follows the translators'
/// `if(date.day)` truthiness on the day explicitly.
pub fn str_to_iso(s: &str, opts: &kovan_common::zotero::date::DateOptions) -> Option<String> {
    let d = kovan_common::zotero::date::str_to_date(s, opts);
    let year = d.year.filter(|y| !y.is_empty())?;
    let mut out = lpad(&year, "0", 4);
    // `parseInt(date.month) == date.month`: true for a numeric month.
    if let Some(m) = d.month {
        out.push('-');
        out.push_str(&lpad(&(m + 1).to_string(), "0", 2));
        if let Some(day) = d.day.filter(|d| *d != 0) {
            out.push('-');
            out.push_str(&lpad(&day.to_string(), "0", 2));
        }
    }
    Some(out)
}

/// `Zotero.Utilities.fieldIsValidForType(field, type)` (:1308-1310).
pub fn field_is_valid_for_type(field: &str, item_type: &str) -> bool {
    match (Field::from_name(field), ItemType::from_name(item_type)) {
        (Some(f), Some(t)) => is_valid_for_type(f, t),
        _ => false,
    }
}

/// `Zotero.Utilities.Item.itemTypeExists` (utilities_item.js:41-49).
pub fn item_type_exists(item_type: &str) -> bool {
    ItemType::from_name(item_type).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_author_matches_upstream_examples() {
        // Expected values follow upstream's code path by hand.
        let a = clean_author("Smith, J. R.", "author", true);
        assert_eq!(a.last_name, "Smith");
        assert_eq!(a.first_name.as_deref(), Some("J. R."));
        let a = clean_author("John Smith", "editor", false);
        assert_eq!(
            (a.first_name.as_deref(), a.last_name.as_str()),
            (Some("John"), "Smith")
        );
        let a = clean_author("Smith, JR", "author", true);
        assert_eq!(a.first_name.as_deref(), Some("J. R."));
        let a = clean_author("Jean-Paul Sartre", "author", false);
        assert_eq!(a.first_name.as_deref(), Some("Jean-Paul"));
        let a = clean_author("Madonna", "author", false);
        assert_eq!(
            (a.first_name.as_deref(), a.last_name.as_str()),
            (Some(""), "Madonna")
        );
    }

    #[test]
    fn clean_doi_cases() {
        assert_eq!(
            clean_doi("doi:10.1234/abc.def.").as_deref(),
            Some("10.1234/abc.def")
        );
        assert_eq!(
            clean_doi("https://doi.org/10.1234%2Fabc").as_deref(),
            Some("10.1234/abc")
        );
        assert_eq!(clean_doi("(10.1234/abc)").as_deref(), Some("10.1234/abc"));
        assert_eq!(clean_doi("10.1234/a(b)").as_deref(), Some("10.1234/a(b)"));
        assert_eq!(clean_doi("nothing"), None);
    }

    #[test]
    fn text2html_and_diacritics() {
        assert_eq!(
            text2html("a & b\n\nc\nd", false),
            "<p>a &amp; b</p><p>c<br/>d</p>"
        );
        assert_eq!(text2html("", false), "<p>&nbsp;</p>");
        assert_eq!(remove_diacritics("Müller Æsir", false), "Muller AEsir");
        assert_eq!(remove_diacritics("Müller Æsir", true), "Muller Æsir");
    }
}
