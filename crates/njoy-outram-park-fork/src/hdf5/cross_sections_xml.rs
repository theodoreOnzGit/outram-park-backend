// SPDX-License-Identifier: GPL-3.0

//! **`cross_sections.xml` — the library index** — GitHub #270 scope item 4.
//!
//! OpenMC locates nuclear data through a small XML index that maps a nuclide
//! or thermal-scattering name to a file, relative to a `<directory>` element.
//! Reading it is what lets a caller say `"U235"` instead of a path.
//!
//! # Why this is hand-parsed rather than `serde-xml-rs`
//!
//! The format is four element types and three attributes. A hand parser that
//! reads exactly those, and **refuses anything it does not recognise**, is
//! less code than the serde derives and — more importantly — cannot silently
//! accept a file whose shape has drifted. A permissive parser that ignores
//! unknown elements would read a *future* `cross_sections.xml` and quietly
//! return an incomplete index.
//!
//! # What this does NOT do
//!
//! It does not read the `.h5` files the index points at. The index is a
//! lookup; resolving an entry to data is the nuclide reader's job.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::NjoyError;

/// What kind of data an entry points at — OpenMC's `type` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LibraryType {
    /// Continuous-energy neutron data.
    Neutron,
    /// Thermal scattering law, `S(alpha, beta)`.
    ThermalScattering,
    /// Photon interaction data.
    Photon,
    /// Windowed multipole.
    Wmp,
}

impl LibraryType {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "neutron" => Some(Self::Neutron),
            "thermal" => Some(Self::ThermalScattering),
            "photon" => Some(Self::Photon),
            "wmp" => Some(Self::Wmp),
            _ => None,
        }
    }
}

/// One `<library>` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    /// The names this file provides — OpenMC's `materials` attribute, which
    /// is a space-separated list and usually holds exactly one name.
    pub materials: Vec<String>,
    /// Path as written in the file, **before** `<directory>` is applied.
    pub path: PathBuf,
    /// What kind of data it is.
    pub kind: LibraryType,
}

/// A parsed `cross_sections.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrossSectionsIndex {
    /// The `<directory>` prefix, if the file had one.
    pub directory: Option<PathBuf>,
    /// Entries in file order.
    pub entries: Vec<LibraryEntry>,
    /// Name -> entry index, built from every entry's `materials` list.
    by_name: BTreeMap<String, usize>,
}

impl CrossSectionsIndex {
    /// Parse from a string.
    ///
    /// # Errors
    ///
    /// An unrecognised element or `type`, a `<library>` missing `materials` or
    /// `path`, or a duplicate name. The last is an error rather than
    /// last-wins: a library that lists `U235` twice is ambiguous, and picking
    /// one silently makes a run depend on file order.
    pub fn parse(xml: &str) -> Result<Self, NjoyError> {
        let mut out = CrossSectionsIndex::default();
        for (i, tag) in tags(xml).into_iter().enumerate() {
            let (name, attrs, text) = tag;
            match name.as_str() {
                "cross_sections" => {}
                "directory" => {
                    out.directory = Some(PathBuf::from(text.trim()));
                }
                "library" => {
                    let materials = attrs.get("materials").ok_or_else(|| {
                        NjoyError::Hdf5(format!(
                            "cross_sections.xml element {i}: <library> has no `materials`"
                        ))
                    })?;
                    let path = attrs.get("path").ok_or_else(|| {
                        NjoyError::Hdf5(format!(
                            "cross_sections.xml element {i}: <library> has no `path`"
                        ))
                    })?;
                    let kind_str = attrs.get("type").map(String::as_str).unwrap_or("neutron");
                    let kind = LibraryType::parse(kind_str).ok_or_else(|| {
                        NjoyError::Hdf5(format!(
                            "cross_sections.xml element {i}: unrecognised type `{kind_str}`. \
                             Refusing rather than skipping: an ignored entry makes this \
                             index silently incomplete."
                        ))
                    })?;
                    let names: Vec<String> =
                        materials.split_whitespace().map(str::to_string).collect();
                    if names.is_empty() {
                        return Err(NjoyError::Hdf5(format!(
                            "cross_sections.xml element {i}: `materials` is empty"
                        )));
                    }
                    let idx = out.entries.len();
                    for n in &names {
                        if out.by_name.insert(n.clone(), idx).is_some() {
                            return Err(NjoyError::Hdf5(format!(
                                "cross_sections.xml lists `{n}` more than once. Picking one \
                                 silently would make a run depend on file order."
                            )));
                        }
                    }
                    out.entries.push(LibraryEntry {
                        materials: names,
                        path: PathBuf::from(path),
                        kind,
                    });
                }
                other => {
                    return Err(NjoyError::Hdf5(format!(
                        "cross_sections.xml element {i}: unrecognised element `{other}`. \
                         A permissive parser would read a future format and return an \
                         incomplete index."
                    )))
                }
            }
        }
        Ok(out)
    }

    /// Parse from a file, defaulting `<directory>` to the file's own folder
    /// when the file does not give one — which is OpenMC's behaviour.
    pub fn read_file(path: &Path) -> Result<Self, NjoyError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            NjoyError::Hdf5(format!("reading {}: {e}", path.display()))
        })?;
        let mut idx = Self::parse(&text)?;
        if idx.directory.is_none() {
            idx.directory = path.parent().map(PathBuf::from);
        }
        Ok(idx)
    }

    /// The resolved path for `name`, or `None` if the index does not carry it.
    pub fn resolve(&self, name: &str) -> Option<PathBuf> {
        let e = &self.entries[*self.by_name.get(name)?];
        Some(match &self.directory {
            Some(d) => d.join(&e.path),
            None => e.path.clone(),
        })
    }

    /// Every name in the index, sorted.
    pub fn names(&self) -> Vec<&str> {
        self.by_name.keys().map(String::as_str).collect()
    }

    /// Entries of one kind.
    pub fn of_kind(&self, kind: LibraryType) -> Vec<&LibraryEntry> {
        self.entries.iter().filter(|e| e.kind == kind).collect()
    }
}

/// Split the document into `(element name, attributes, text)` triples.
///
/// Handles exactly what `cross_sections.xml` uses: self-closing elements with
/// attributes, and elements whose only content is text. Comments and the XML
/// declaration are skipped. Anything else falls through to the caller's
/// unrecognised-element error, which is the intent.
fn tags(xml: &str) -> Vec<(String, BTreeMap<String, String>, String)> {
    let mut out = Vec::new();
    let bytes: Vec<char> = xml.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != '<' {
            i += 1;
            continue;
        }
        // Skip comments and declarations.
        if xml[byte_at(&bytes, i)..].starts_with("<!--") {
            match xml[byte_at(&bytes, i)..].find("-->") {
                Some(off) => {
                    let abs = byte_at(&bytes, i) + off + 3;
                    i = char_at(xml, abs);
                    continue;
                }
                None => break,
            }
        }
        if bytes[i + 1] == '?' || bytes[i + 1] == '!' || bytes[i + 1] == '/' {
            while i < bytes.len() && bytes[i] != '>' {
                i += 1;
            }
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != '>' {
            j += 1;
        }
        let inner: String = bytes[start..j].iter().collect();
        let self_closing = inner.trim_end().ends_with('/');
        let inner = inner.trim_end().trim_end_matches('/');
        let mut parts = inner.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim().to_string();
        let attrs = parse_attrs(parts.next().unwrap_or(""));

        // Text content, for a non-self-closing element.
        let mut text = String::new();
        i = j + 1;
        if !self_closing {
            let mut k = i;
            while k < bytes.len() && bytes[k] != '<' {
                text.push(bytes[k]);
                k += 1;
            }
            // Only consume the text if what follows closes THIS element;
            // otherwise leave the position alone so nested elements are seen.
            let rest: String = bytes[k..].iter().take(name.len() + 3).collect();
            if rest.starts_with(&format!("</{name}")) {
                i = k;
            } else {
                text.clear();
            }
        }
        if !name.is_empty() {
            out.push((name, attrs, text));
        }
    }
    out
}

fn byte_at(chars: &[char], i: usize) -> usize {
    chars[..i].iter().map(|c| c.len_utf8()).sum()
}

fn char_at(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}

fn parse_attrs(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let key: String = chars[start..i].iter().collect();
        while i < chars.len() && (chars[i] == '=' || chars[i].is_whitespace()) {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let quote = chars[i];
        if quote != '"' && quote != '\'' {
            break;
        }
        i += 1;
        let vstart = i;
        while i < chars.len() && chars[i] != quote {
            i += 1;
        }
        let value: String = chars[vstart..i].iter().collect();
        i += 1;
        if !key.is_empty() {
            out.insert(key, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version='1.0' encoding='UTF-8'?>
<cross_sections>
  <directory>/data/endfb80_hdf5</directory>
  <!-- a comment -->
  <library materials="U235" path="U235.h5" type="neutron"/>
  <library materials="U238" path="U238.h5" type="neutron"/>
  <library materials="c_H_in_H2O" path="c_H_in_H2O.h5" type="thermal"/>
  <library materials="U235" path="wmp/092235.h5" type="wmp"/>
</cross_sections>
"#;

    /// The index resolves a name to a path under `<directory>`.
    #[test]
    fn the_index_resolves_names_to_paths() {
        // The duplicate U235 (neutron + wmp) is refused, so drop the wmp line
        // for the happy path and test the duplicate separately.
        let xml = SAMPLE.replace(
            "  <library materials=\"U235\" path=\"wmp/092235.h5\" type=\"wmp\"/>\n",
            "",
        );
        let idx = CrossSectionsIndex::parse(&xml).unwrap();
        assert_eq!(idx.directory, Some(PathBuf::from("/data/endfb80_hdf5")));
        assert_eq!(idx.entries.len(), 3);
        assert_eq!(
            idx.resolve("U238"),
            Some(PathBuf::from("/data/endfb80_hdf5/U238.h5"))
        );
        assert_eq!(
            idx.resolve("c_H_in_H2O"),
            Some(PathBuf::from("/data/endfb80_hdf5/c_H_in_H2O.h5"))
        );
        assert_eq!(idx.resolve("Pu239"), None);
        assert_eq!(idx.names(), vec!["U235", "U238", "c_H_in_H2O"]);
        assert_eq!(idx.of_kind(LibraryType::Neutron).len(), 2);
        assert_eq!(idx.of_kind(LibraryType::ThermalScattering).len(), 1);
    }

    /// **A duplicate name is refused, not last-wins.** Picking one silently
    /// would make a run depend on the order of lines in a data file.
    #[test]
    fn a_duplicate_name_is_refused() {
        let err = CrossSectionsIndex::parse(SAMPLE).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("more than once"), "{msg}");
    }

    /// **An unrecognised element or type is refused, not skipped.** A
    /// permissive parser would read a future format and return an index that
    /// is silently missing entries.
    #[test]
    fn unrecognised_content_is_refused_rather_than_skipped() {
        let err = CrossSectionsIndex::parse(
            "<cross_sections><brand_new_element foo=\"1\"/></cross_sections>",
        )
        .unwrap_err();
        assert!(format!("{err}").contains("unrecognised element"), "{err}");

        let err = CrossSectionsIndex::parse(
            "<cross_sections><library materials=\"X\" path=\"x.h5\" type=\"quark\"/></cross_sections>",
        )
        .unwrap_err();
        assert!(format!("{err}").contains("unrecognised type"), "{err}");
    }

    /// Missing required attributes are named in the error.
    #[test]
    fn missing_attributes_are_named() {
        let err =
            CrossSectionsIndex::parse("<cross_sections><library path=\"x.h5\"/></cross_sections>")
                .unwrap_err();
        assert!(format!("{err}").contains("`materials`"), "{err}");
        let err = CrossSectionsIndex::parse(
            "<cross_sections><library materials=\"X\"/></cross_sections>",
        )
        .unwrap_err();
        assert!(format!("{err}").contains("`path`"), "{err}");
    }

    /// A `materials` attribute may list several names, all of which resolve to
    /// the same file.
    #[test]
    fn one_entry_can_provide_several_names() {
        let idx = CrossSectionsIndex::parse(
            "<cross_sections><library materials=\"Am242 Am242_m1\" path=\"Am242.h5\" \
             type=\"neutron\"/></cross_sections>",
        )
        .unwrap();
        assert_eq!(idx.entries.len(), 1);
        assert_eq!(idx.resolve("Am242"), Some(PathBuf::from("Am242.h5")));
        assert_eq!(idx.resolve("Am242_m1"), Some(PathBuf::from("Am242.h5")));
    }

    /// Single-quoted attributes and a missing `type` (defaulting to neutron)
    /// both work — both appear in real files.
    #[test]
    fn single_quotes_and_a_defaulted_type_are_accepted() {
        let idx = CrossSectionsIndex::parse(
            "<cross_sections><library materials='H1' path='H1.h5'/></cross_sections>",
        )
        .unwrap();
        assert_eq!(idx.entries[0].kind, LibraryType::Neutron);
        assert_eq!(idx.resolve("H1"), Some(PathBuf::from("H1.h5")));
    }
}
