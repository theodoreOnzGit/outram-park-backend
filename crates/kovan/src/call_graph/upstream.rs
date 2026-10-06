//! The **upstream counterpart** of a ported source file, read from its
//! attribution header (GitHub #746, part 2).
//!
//! The workspace rule (root `CLAUDE.md`, "Preserve GPLv3 compatibility and
//! provenance headers") is that a file porting from an upstream project
//! keeps an attribution header naming the upstream project, source file,
//! version or commit, copyright and licence. This module reads that header
//! into an [`Upstream`] record. Plain `std`, no I/O.
//!
//! # What is read
//!
//! Only the file's **leading `//` comment block**: the lines before the
//! first line that is not a `//` comment or blank (`//!` and `///` doc
//! comments end it). Provenance written only in a `//!` doc comment ("Ported
//! from `bsigma` in NJOY2016 `broadr.f90`", most of `njoy-outram-park-fork`)
//! and attribution blocks further down a file (a separator-headed block
//! introducing one ported routine) are not read; [`scan`] reports the first
//! as [`Scan::Unparsed`] when the doc comment says "ported from", "port of"
//! or "upstream commit" (a flag for a person to look at, which also catches
//! prose such as "a port of physics the maintainer has validated"). A
//! negated marker ("not a port of the PANAMA Fortran", boon-lay's
//! `fuel_failure/`) is not an attribution.
//!
//! # The styles found (surveyed 2026-10-06 over `crates/`)
//!
//! 1. **Key-value**, `Upstream <key> : <value>`, aligned or not, with the
//!    value continuing on more-indented lines. Keys seen: `project`, `URL`,
//!    `repo`, `commit`, `version`, `source`, `source files`, `file`,
//!    `licence`/`license`, `copyright`, `author`; also a bare `Upstream:
//!    <url>` (GeN-Foam ports). Examples: `boon-lay` `triso_atops_fork/`
//!    (TRISO-ATOPS), `changi` `flexpart/` (FLEXPART, commit inside the
//!    version: `10.4 (2019-11-12), commit 3d7eebf`), `raffles` `scram/`,
//!    `outram-foam-*` GeN-Foam and OpenFOAM-dev ports, `cyclus` ports.
//! 2. **Prose**, `Ported from <project> …` / `Derived from <project> (<url>),
//!    upstream commit <sha>` / `PORTED from …`. Examples: `njoy-outram-park-fork`
//!    (``Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5…)``),
//!    `outram-park-fork-offbeat` (`Derived from OFFBEAT (https://gitlab.com/…),
//!    upstream commit 80e8445…`), `outram-mc-libs`/`outram-blender`
//!    (`Ported from OpenMC (https://github.com/openmc-dev/openmc, …): src/mesh.cpp`),
//!    `petir` (`Ported from the GNU Scientific Library `cheb/` module (cheb/init.c,
//!    …), GSL 2.8 at commit cf180cd…`).
//!
//! Key-value fields win over prose where both are present.
//!
//! # The link: never guessed
//!
//! [`link`] builds a URL only when the header records **both** a repository
//! URL on `github.com` or `gitlab.com` (the two hosts whose URL scheme is
//! known) **and** a commit hash (7-40 hex digits; a version number is
//! recorded but not turned into a tag, since the tag's spelling is not
//! recorded). It links the file (`…/blob/<commit>/<path>`) when the header
//! names exactly one file with a directory part, else the tree at the commit.
//! A project with no recorded repository URL (NJOY2016's headers record the
//! commit but not the repository) gets its fields and no URL.

use serde::{Deserialize, Serialize};

/// How the header was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderStyle {
    /// `Upstream <key>: <value>` lines.
    KeyValue,
    /// `Ported from <project> …` / `Derived from <project> …` sentences.
    Prose,
}

/// A file's upstream counterpart, as its attribution header records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upstream {
    pub style: HeaderStyle,
    /// 1-based line where the attribution starts.
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The repository URL as recorded (`<>` and a trailing `.git`/`/` removed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The commit hash as recorded (7-40 hex digits).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The recorded upstream source text, continuation lines joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The file paths found in `source` (line suffixes `:12-40` removed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    /// Set only by [`link`]'s rule: recorded repository on a known host and a
    /// recorded commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// What [`scan`] found in a file's header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scan {
    /// No attribution marker: not a port, or not declared as one.
    None,
    Parsed(Upstream),
    /// A provenance marker was seen but no project or repository could be
    /// read; the string says where (`doc comment: …` or `header: …`).
    Unparsed(String),
}

const FILE_EXTS: &[&str] = &[
    "f90", "f", "F90", "F", "f77", "for", "inc", "c", "h", "cc", "cpp", "cxx", "hpp", "hh", "H",
    "C", "py", "m", "vb", "cs", "java", "jl", "js", "ts", "rs",
];

/// The leading `//` comment block: `(0-based line, text after "//")`.
fn header(lines: &[String]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("//!") || t.starts_with("///") || !t.starts_with("//") {
            break;
        }
        out.push((k, t[2..].to_string()));
    }
    out
}

fn indent(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

fn is_hex_commit(t: &str) -> bool {
    (7..=40).contains(&t.len())
        && t.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        && t.chars().any(|c| c.is_ascii_digit())
}

/// The first commit-shaped token in `s`.
fn find_commit(s: &str) -> Option<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .find(|t| is_hex_commit(t))
        .map(str::to_string)
}

/// The first `http(s)://` URL in `s`, trimmed of surrounding punctuation.
fn find_url(s: &str) -> Option<String> {
    let i = s.find("http://").or_else(|| s.find("https://"))?;
    let rest = &s[i..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '>' | ')' | ',' | ';' | '`' | '"'))
        .unwrap_or(rest.len());
    let u = rest[..end].trim_end_matches(['.', ':']);
    Some(normalise_repo(u))
}

fn normalise_repo(u: &str) -> String {
    let u = u.trim().trim_start_matches('<').trim_end_matches('>');
    let u = u.trim_end_matches('/');
    u.strip_suffix(".git").unwrap_or(u).to_string()
}

/// File paths in a source description.
fn find_files(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in s.split(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')' | '`' | ';')) {
        if raw.contains("://") {
            continue;
        }
        let t = raw.split(':').next().unwrap_or("");
        let t = t.trim_end_matches('.');
        let Some((stem, ext)) = t.rsplit_once('.') else {
            continue;
        };
        let ok_chars = t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'));
        if ok_chars && !stem.is_empty() && FILE_EXTS.contains(&ext) && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// A key-value line: the normalised key and the value.
fn key_value(text: &str) -> Option<(String, String)> {
    let (k, v) = text.split_once(':')?;
    // `https://…` is a value, not a key.
    if v.starts_with("//") {
        return None;
    }
    let key: String = k
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let rest = key.strip_prefix("upstream").map(str::trim)?;
    let field = match rest {
        // A bare `Upstream:` holds a URL (GeN-Foam's lib.rs) or a path
        // (GeN-Foam's boundary conditions); `scan` routes it by content.
        "" => "bare",
        "url" | "repo" | "repository" => "repository",
        "project" => "project",
        "commit" => "commit",
        "version" => "version",
        "source" | "sources" | "source file" | "source files" | "file" | "files" => "source",
        "licence" | "license" => "licence",
        _ => return None,
    };
    Some((field.to_string(), v.trim().to_string()))
}

const PROSE_MARKERS: &[&str] = &[
    "ported from ",
    "derived from ",
    "translated from ",
    "port of ",
];

/// The first marker not negated ("not a port of", "never ported from").
fn prose_marker(lower: &str) -> Option<(usize, usize)> {
    PROSE_MARKERS
        .iter()
        .flat_map(|m| lower.match_indices(m).map(|(i, _)| (i, m.len())))
        .filter(|&(i, _)| {
            let before = lower[..i].trim_end();
            let before = before.strip_suffix(" a").unwrap_or(before);
            !["not", "never", "no", "nor"]
                .iter()
                .any(|n| before == *n || before.ends_with(&format!(" {n}")))
        })
        .min()
}

/// Markers that flag provenance written only in a `//!` doc comment.
const DOC_MARKERS: &[&str] = &["ported from ", "port of ", "upstream commit"];

/// Reads the attribution header of a file's lines.
pub fn scan(lines: &[String]) -> Scan {
    let hdr = header(lines);
    let mut up = Upstream {
        style: HeaderStyle::KeyValue,
        line: 0,
        project: None,
        repository: None,
        version: None,
        commit: None,
        source: None,
        files: Vec::new(),
        licence: None,
        url: None,
    };
    let mut first_kv: Option<usize> = None;
    // Key-value pass.
    let mut k = 0;
    while k < hdr.len() {
        let (line, text) = &hdr[k];
        let Some((field, mut value)) = key_value(text.trim()) else {
            k += 1;
            continue;
        };
        first_kv.get_or_insert(*line);
        let ind = indent(text);
        let mut j = k + 1;
        while j < hdr.len() {
            let t = &hdr[j].1;
            if t.trim().is_empty() || indent(t) <= ind || key_value(t.trim()).is_some() {
                break;
            }
            value.push(' ');
            value.push_str(t.trim());
            j += 1;
        }
        k = j;
        let slot = match field.as_str() {
            "repository" => {
                if let Some(u) = find_url(&value) {
                    up.repository.get_or_insert(u);
                }
                continue;
            }
            "bare" => {
                if let Some(u) = find_url(&value) {
                    up.repository.get_or_insert(u);
                    continue;
                }
                up.files.extend(find_files(&value));
                &mut up.source
            }
            "project" => {
                if let Some(u) = find_url(&value) {
                    up.repository.get_or_insert(u);
                }
                let name = project_name(&value);
                if !name.is_empty() {
                    up.project.get_or_insert(name);
                }
                continue;
            }
            "commit" => {
                if let Some(c) = find_commit(&value) {
                    up.commit.get_or_insert(c);
                }
                continue;
            }
            "version" => {
                if up.commit.is_none() {
                    up.commit = find_commit(&value);
                }
                &mut up.version
            }
            "source" => {
                up.files.extend(find_files(&value));
                &mut up.source
            }
            _ => &mut up.licence,
        };
        match slot {
            Some(s) => {
                s.push_str("; ");
                s.push_str(&value);
            }
            None => *slot = Some(value),
        }
    }
    // Prose pass: the first marker sentence, up to a blank comment line.
    let mut prose_line: Option<usize> = None;
    let mut sentence = String::new();
    for (i, (line, text)) in hdr.iter().enumerate() {
        let lower = text.to_lowercase();
        if let Some((at, len)) = prose_marker(&lower) {
            prose_line = Some(*line);
            sentence.push_str(&text[at + len..]);
            for (_, t) in &hdr[i + 1..] {
                if t.trim().is_empty() {
                    break;
                }
                sentence.push(' ');
                sentence.push_str(t.trim());
            }
            break;
        }
    }
    if prose_line.is_some() {
        if up.project.is_none() {
            let name = project_name(&sentence);
            if !name.is_empty() {
                up.project = Some(name);
            }
        }
        if up.repository.is_none() {
            up.repository = find_url(&sentence).filter(|u| !u.contains("gnu.org/licenses"));
        }
        if up.commit.is_none() {
            if let Some(i) = sentence.to_lowercase().find("commit") {
                up.commit = find_commit(&sentence[i..]);
            }
        }
        if up.files.is_empty() {
            up.files = find_files(&sentence);
        }
        if first_kv.is_none() {
            up.style = HeaderStyle::Prose;
        }
    }
    up.line = first_kv.or(prose_line).map_or(0, |l| l as u32 + 1);
    if up.project.is_some() || up.repository.is_some() {
        up.url = link(&up);
        return Scan::Parsed(up);
    }
    if let Some(l) = first_kv.or(prose_line) {
        let text = &lines[l];
        return Scan::Unparsed(format!("header: {}", text.trim()));
    }
    // A marker in the module doc comment only.
    for l in lines.iter().take(80) {
        let t = l.trim_start();
        if let Some(doc) = t.strip_prefix("//!") {
            let lower = doc.to_lowercase();
            if DOC_MARKERS.iter().any(|m| lower.contains(m)) {
                return Scan::Unparsed(format!("doc comment: {}", doc.trim()));
            }
        } else if !t.is_empty() && !t.starts_with("//") && !t.starts_with("#!") {
            break;
        }
    }
    Scan::None
}

/// The project's name from a value or a prose sentence: the text before the
/// first `(`, `<`, `,`, `:`, `—`, backtick or URL, with a leading "the "
/// dropped.
fn project_name(v: &str) -> String {
    let v = v.trim().replace('`', "");
    let mut v = v.as_str();
    for lead in ["the ", "The "] {
        if let Some(rest) = v.strip_prefix(lead) {
            v = rest;
        }
    }
    // An in-workspace origin ("this workspace's `x`") is not an upstream.
    if ["this ", "This ", "our ", "Our "]
        .iter()
        .any(|p| v.starts_with(p))
    {
        return String::new();
    }
    let end = v
        .find(|c: char| matches!(c, '(' | '<' | ',' | ':' | '`' | '—' | ';'))
        .unwrap_or(v.len());
    let end = v.find("http").map_or(end, |h| h.min(end));
    // A sentence end ("… `BLI_polyfill_calc`. Upstream commit …").
    let end = v.find(". ").map_or(end, |h| h.min(end));
    let name = v[..end]
        .trim()
        .trim_end_matches(['-', '.'])
        .trim()
        .to_string();
    // A sentence that runs on ("NJOY2016 src/x.f90 …"): keep the first words
    // up to one that looks like a path.
    let words: Vec<&str> = name
        .split_whitespace()
        .take_while(|w| !w.contains('/') && find_files(w).is_empty())
        .collect();
    words.join(" ")
}

/// The upstream URL, under the rule in the module doc: known host and a
/// recorded commit, else `None`.
pub fn link(up: &Upstream) -> Option<String> {
    let repo = up.repository.as_deref()?;
    let commit = up.commit.as_deref()?;
    let rest = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let segs = path.split('/').filter(|s| !s.is_empty()).count();
    let (blob, tree) = match host {
        "github.com" if segs == 2 => ("blob", "tree"),
        "gitlab.com" if segs >= 2 => ("-/blob", "-/tree"),
        _ => return None,
    };
    let base = format!("https://{host}/{}", path.trim_matches('/'));
    match up.files.as_slice() {
        [one] if one.contains('/') => Some(format!(
            "{base}/{blob}/{commit}/{}",
            one.trim_start_matches("./").trim_start_matches('/')
        )),
        _ => Some(format!("{base}/{tree}/{commit}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    fn parsed(s: &str) -> Upstream {
        match scan(&lines(s)) {
            Scan::Parsed(u) => u,
            other => panic!("not parsed: {other:?}"),
        }
    }

    /// Methodology: the TRISO-ATOPS key-value header (boon-lay), verbatim
    /// in shape: project with an em-dash URL, short commit, one source file
    /// with a continuation line.
    ///
    /// Result (2026-10-06): passes; the URL links the file at the commit.
    #[test]
    fn key_value_triso_atops() {
        let u = parsed(
            "// SPDX-License-Identifier: GPL-3.0\n//\n// TRISO-ATOPS fork — provenance\n// -----------------------------\n// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS\n// Upstream commit  : de374c8\n// Upstream source  : trisoatops/utility_functions/calculation_functions.py\n//                    (`inventory_processing`, `release_activity`)\n// Original license : MIT\n\n//! Doc.\nfn x() {}\n",
        );
        assert_eq!(u.style, HeaderStyle::KeyValue);
        assert_eq!(u.line, 5);
        assert_eq!(u.project.as_deref(), Some("TRISO-ATOPS"));
        assert_eq!(
            u.repository.as_deref(),
            Some("https://github.com/IdahoLabResearch/TRISO-ATOPS")
        );
        assert_eq!(u.commit.as_deref(), Some("de374c8"));
        assert_eq!(
            u.files,
            vec!["trisoatops/utility_functions/calculation_functions.py"]
        );
        assert_eq!(
            u.url.as_deref(),
            Some("https://github.com/IdahoLabResearch/TRISO-ATOPS/blob/de374c8/trisoatops/utility_functions/calculation_functions.py")
        );
    }

    /// Methodology: FLEXPART's header (changi) records the commit inside the
    /// version and two files: the URL is the tree at the commit.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn key_value_commit_in_version_two_files() {
        let u = parsed(
            "// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart\n// Upstream version : 10.4 (2019-11-12), commit 3d7eebf\n// Upstream source  : src/juldate.f90, src/caldate.f90 (behaviour only)\n",
        );
        assert_eq!(
            u.version.as_deref(),
            Some("10.4 (2019-11-12), commit 3d7eebf")
        );
        assert_eq!(u.commit.as_deref(), Some("3d7eebf"));
        assert_eq!(u.files, vec!["src/juldate.f90", "src/caldate.f90"]);
        assert_eq!(
            u.url.as_deref(),
            Some("https://github.com/flexpart/flexpart/tree/3d7eebf")
        );
    }

    /// Methodology: GeN-Foam's indented `Upstream: <gitlab url>` form.
    ///
    /// Result (2026-10-06): passes; gitlab's `/-/tree/` form.
    #[test]
    fn key_value_bare_upstream_gitlab() {
        let u = parsed(
            "// SPDX-License-Identifier: GPL-3.0-only\n//\n// Derived from GeN-Foam (Generalized Nuclear Foam)\n//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam\n//   Upstream commit: 652b3da\n//   Upstream license: GPL-3.0\n",
        );
        assert_eq!(u.project.as_deref(), Some("GeN-Foam"));
        assert_eq!(u.licence.as_deref(), Some("GPL-3.0"));
        // The other GeN-Foam form: a bare `Upstream:` holding the path.
        let b = parsed(
            "// Derived from GeN-Foam (Generalized Nuclear Foam)\n//   Upstream: src/classes/a/B.C\n//   Upstream commit: 652b3da\n",
        );
        assert_eq!(
            (b.repository.as_deref(), b.files.clone()),
            (None, vec!["src/classes/a/B.C".to_string()])
        );
        assert_eq!(b.url, None);
        assert_eq!(
            u.url.as_deref(),
            Some("https://gitlab.com/foam-for-nuclear/GeN-Foam/-/tree/652b3da")
        );
    }

    /// Methodology: NJOY2016's prose header records the file and commit but
    /// no repository URL, so no link is built (never guessed).
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn prose_without_repository_has_no_url() {
        let u = parsed(
            "// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).\nuse x;\n",
        );
        assert_eq!(u.style, HeaderStyle::Prose);
        assert_eq!(u.project.as_deref(), Some("NJOY2016"));
        assert_eq!(u.files, vec!["src/groupr.f90"]);
        assert_eq!(
            u.commit.as_deref(),
            Some("ac5adf5f33d893e42f2eed7fb286b0d51c7580da")
        );
        assert_eq!(u.url, None);
    }

    /// Methodology: OFFBEAT's prose (`Derived from X (url), upstream commit
    /// sha`), and OpenMC's with a URL but no commit (no link).
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn prose_with_and_without_commit() {
        let u = parsed(
            "// SPDX-License-Identifier: GPL-3.0-only\n//\n// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),\n// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.\n",
        );
        assert_eq!(u.project.as_deref(), Some("OFFBEAT"));
        assert_eq!(
            u.url.as_deref(),
            Some("https://gitlab.com/foam-for-nuclear/offbeat/-/tree/80e84450a115b0c411e1bfa5d166379f6bf6c084")
        );
        let o = parsed(
            "// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence):\n//   src/mesh.cpp, include/openmc/mesh.h\n",
        );
        assert_eq!(o.project.as_deref(), Some("OpenMC"));
        assert_eq!(
            o.repository.as_deref(),
            Some("https://github.com/openmc-dev/openmc")
        );
        assert_eq!(o.files, vec!["src/mesh.cpp", "include/openmc/mesh.h"]);
        assert_eq!(o.commit, None);
        assert_eq!(o.url, None);
        // Backticked and in-workspace origins.
        let p = parsed("// PORTED from the `peroxide` crate, version 0.41.2, file src/x.rs\n");
        assert_eq!(p.project.as_deref(), Some("peroxide crate"));
        assert_eq!(
            scan(&lines(
                "// Layout ported from this workspace's `outram_foam_basic_lib::interface`\n"
            )),
            Scan::Unparsed(
                "header: // Layout ported from this workspace's `outram_foam_basic_lib::interface`"
                    .into()
            )
        );
    }

    /// Methodology: a plain SPDX header is not a port; provenance only in a
    /// `//!` doc comment is reported unparsed, not guessed.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn none_and_doc_only() {
        assert_eq!(
            scan(&lines("// SPDX-License-Identifier: GPL-3.0-only\n// Copyright (C) 2026 OUTRAM PARK contributors\n\n//! Doc.\n")),
            Scan::None
        );
        let s = scan(&lines(
            "//! BROADR.\n//!\n//! Ported from `bsigma` in NJOY2016 `broadr.f90`.\nuse x;\n",
        ));
        assert!(
            matches!(s, Scan::Unparsed(ref r) if r.starts_with("doc comment:")),
            "{s:?}"
        );
        // A negated marker is not an attribution (boon-lay's PANAMA-I header).
        assert_eq!(
            scan(&lines("// Nature : boon-lay's own model; not a port of the PANAMA Fortran\n// (closed-source, never consulted).\n")),
            Scan::None
        );
    }

    /// Methodology: an unknown host (codeberg) with a commit gets no URL.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn unknown_host_has_no_url() {
        let u = parsed(
            "// Upstream project: X <https://codeberg.org/a/b>\n// Upstream commit: 1234abc\n",
        );
        assert_eq!(u.repository.as_deref(), Some("https://codeberg.org/a/b"));
        assert_eq!(u.url, None);
    }
}
