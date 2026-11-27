// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// Port of the ASCII field/dictionary write of OpenFOAM-ESI v2506:
//   src/OpenFOAM/db/IOstreams/Sstreams/OSstream.C            write(double)
//   src/OpenFOAM/containers/Lists/List/UListIO.C             UList::writeList, UList::writeEntry
//   src/OpenFOAM/fields/Fields/Field/Field.C                 Field::writeEntry (uniform / nonuniform)
//   src/OpenFOAM/db/IOobject/IOobjectWriteHeader.C           banner, FoamFile header
//   src/OpenFOAM/fields/GeometricFields/GeometricField/      GeometricField::writeData / boundaryField
//   src/OpenFOAM/db/Time/Time.C, TimeIO.C                    Time::timeName, Time::writeTimeDict
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # `foam_ascii` — OpenFOAM's ASCII time-directory text, byte for byte (gh:#1931)
//!
//! The text GeN-Foam's `runTime.write()` puts in a time directory: the banner and `FoamFile`
//! header, `dimensions`, `internalField` (`uniform` or `nonuniform List<T>`), `boundaryField`, the
//! `scalarFieldField`/`vectorField` lists and the `uniform/time` and `uniform/reactorState`
//! dictionaries. Pure string building (`no_std` + `alloc`, no file I/O): the caller writes the
//! text where it likes.
//!
//! ## Number format
//!
//! `OSstream::write(double)` is `os_ << val` with the stream's `precision()` (`writePrecision`)
//! and the default float format, which is C's `%.{p}g`: [`format_g`]. Rust's `{:e}` is correctly
//! rounded (round half to even on the exact binary value), as glibc's `printf` is, so the digits
//! agree; the layout (fixed below `1e-4`/above `10^p`, trailing zeros stripped, `e+07` with at
//! least two exponent digits, `-0`) is `%g`'s.
//!
//! ## Lists
//!
//! `UList::writeList` (`UListIO.C:81-170`): more than one entry and all equal -> `N{x}`; at most
//! one entry, or at most 10 entries of a contiguous type -> `N(a b c)`; otherwise
//! `\nN\n(\nx\ny\n)\n`. A field entry is `uniform x` when the list is non-empty and every entry
//! equals the first (`Field::writeEntry`), else `nonuniform List<type> ` and the list.
//!
//! ## Refused by name
//!
//! Binary format, `writeCompression on`, `timeFormat` other than `general` ([`WriteOptions`] has
//! no field for them: asking for them is a refusal in the caller), and every patch type that is
//! not in [`SUPPORTED_PATCH_TYPES`] ([`write_field_file`] returns [`FoamWriteError::PatchType`]).
//!
//! V&V: `tests/genfoam_written_state_vs_upstream.rs` (README "Round 68"): every number token of
//! the real written directory reformatted, every field file re-written byte for byte.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use core::fmt::Write as _;

/// `foamVersion::buildArch` of the real binary: `label=32`, `scalar=64`, little-endian.
pub const ARCH: &str = "LSB;label=32;scalar=64";

/// The patch types the writer knows how to lay out: those the `2D_fullCoupling` and `2D_FFTF` cases write.
/// Any other type is refused by name ([`FoamWriteError::PatchType`]).
pub const SUPPORTED_PATCH_TYPES: [&str; 8] =
    ["empty", "zeroGradient", "slip", "calculated", "fixedValue", "fixedFluxPressure", "GFTractionDisplacement", "mapped"];

/// `Time::maxPrecision_` = `3 - log10(SMALL)` with `SMALL = 1e-15`: the digits of `uniform/time`'s `value`.
pub const MAX_TIME_PRECISION: usize = 18;

/// `Foam::IOobject::writeBanner` for version 2506.
const BANNER: &str = "/*--------------------------------*- C++ -*----------------------------------*\\\n\
| =========                 |                                                 |\n\
| \\\\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox           |\n\
|  \\\\    /   O peration     | Version:  2506                                  |\n\
|   \\\\  /    A nd           | Website:  www.openfoam.com                      |\n\
|    \\\\/     M anipulation  |                                                 |\n\
\\*---------------------------------------------------------------------------*/\n";

/// `Foam::IOobject::writeEndDivider`, with the two newlines `regIOobject::writeObject` puts before it.
const FOOTER: &str = "\n\n// ************************************************************************* //\n";

/// [`FOOTER`] for the sibling writers (`foam_ascii_mesh`).
pub(crate) fn footer() -> &'static str {
    FOOTER
}

/// `keyword<pad>value;\n` at `indent`, for the sibling writers.
pub(crate) fn statement_text(indent: usize, key: &str, value: &str) -> String {
    statement(indent, key, value)
}

/// What `controlDict` fixes about the text: `writePrecision` (`writeFormat ascii`, `writeCompression off`, `timeFormat general` are not options).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOptions {
    /// `writePrecision` (`IOstream::defaultPrecision()`): significant digits of every number.
    pub precision: usize,
}

impl WriteOptions {
    /// `writePrecision 17`: the round-trip precision of a double, as the `2D_fullCoupling` cases set.
    #[must_use]
    pub fn precision_17() -> Self {
        Self { precision: 17 }
    }
}

/// Why a file could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoamWriteError {
    /// A patch field type the writer does not lay out (refused by name).
    PatchType(String),
    /// A list whose length is not a multiple of the component count.
    Shape(String),
    /// A mesh object the writer refuses by name (`foam_ascii_mesh`).
    Mesh(String),
}

impl core::fmt::Display for FoamWriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PatchType(t) => write!(f, "patch field type `{t}` is not written by this port (supported: {SUPPORTED_PATCH_TYPES:?})"),
            Self::Shape(s) => write!(f, "malformed field data: {s}"),
            Self::Mesh(s) => write!(f, "{s}"),
        }
    }
}

/// `os << double` at `precision` significant digits: C's `%.{precision}g` (the C++ stream default float format).
///
/// `precision` 0 is treated as 1 (`%g`). Not-a-number and infinities print as `nan`, `inf`, `-inf`.
#[must_use]
pub fn format_g(x: f64, precision: usize) -> String {
    let p = precision.max(1);
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan".to_string() } else { "nan".to_string() };
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf".to_string() } else { "inf".to_string() };
    }
    let neg = x.is_sign_negative();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if x == 0.0 {
        out.push('0');
        return out;
    }
    // `{:.*e}` gives `d.ddd...e<exp>` with p significant digits, correctly rounded
    let sci = format!("{:.*e}", p - 1, x.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let strip = |s: &str| s.trim_end_matches('0').to_string();
    if exp < -4 || exp >= p as i32 {
        // scientific: d[.ddd]e+XX
        out.push_str(&digits[..1]);
        let frac = strip(&digits[1..]);
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        let _ = write!(out, "{:02}", exp.abs());
    } else if exp >= 0 {
        let int_len = exp as usize + 1;
        out.push_str(&digits[..int_len]);
        let frac = strip(&digits[int_len..]);
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
    } else {
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        out.push_str(&strip(&digits));
    }
    out
}

/// `Time::timeName(t, precision)`: the time-directory name (`timeFormat general`, `timePrecision` digits).
#[must_use]
pub fn time_name(t: f64, precision: usize) -> String {
    format_g(t, precision)
}

/// A field's values: `n_cmpt` components per entry (1 scalar, 3 vector, 6 symmTensor, 9 tensor), flat.
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    /// Components per entry.
    pub n_cmpt: usize,
    /// `len x n_cmpt` values, entry-major.
    pub values: Vec<f64>,
}

impl Data {
    /// A scalar list.
    #[must_use]
    pub fn scalar(values: Vec<f64>) -> Self {
        Self { n_cmpt: 1, values }
    }

    /// A vector list.
    #[must_use]
    pub fn vector(values: &[[f64; 3]]) -> Self {
        Self { n_cmpt: 3, values: values.iter().flat_map(|v| v.iter().copied()).collect() }
    }

    /// A list of `n_cmpt`-component entries from a flat array.
    #[must_use]
    pub fn flat(n_cmpt: usize, values: Vec<f64>) -> Self {
        Self { n_cmpt, values }
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len() / self.n_cmpt.max(1)
    }

    /// True if there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// `pTraits<T>::typeName`: `scalar`, `vector`, `symmTensor`, `tensor`.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self.n_cmpt {
            1 => "scalar",
            3 => "vector",
            6 => "symmTensor",
            _ => "tensor",
        }
    }

    fn entry(&self, i: usize) -> &[f64] {
        &self.values[i * self.n_cmpt..(i + 1) * self.n_cmpt]
    }

    /// `List::uniform()`: non-empty and every entry equals the first (`==`, so `-0 == 0`).
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        let n = self.len();
        n > 0 && (1..n).all(|i| self.entry(i) == self.entry(0))
    }

    fn element_text(&self, i: usize, precision: usize) -> String {
        element_text(self.entry(i), precision)
    }

    /// `(a b c)` of one entry, or the bare number of a scalar.
    fn front_text(&self, precision: usize) -> String {
        self.element_text(0, precision)
    }
}

fn element_text(e: &[f64], precision: usize) -> String {
    if e.len() == 1 {
        return format_g(e[0], precision);
    }
    let mut s = String::from("(");
    for (k, v) in e.iter().enumerate() {
        if k > 0 {
            s.push(' ');
        }
        s.push_str(&format_g(*v, precision));
    }
    s.push(')');
    s
}

/// `UList<T>::writeList` for already-rendered entries (`UListIO.C:81-170`), ASCII.
///
/// `contiguous` is `is_contiguous_v<T>` (numbers and vectors: true; a list of lists: false), `all_equal` whether the entries are
/// identical (`list.uniform()`), `short_len` the policy's `short_length` (10).
#[must_use]
pub fn write_list_text(entries: &[String], contiguous: bool, all_equal: bool, short_len: usize) -> String {
    let len = entries.len();
    let mut s = String::new();
    if contiguous && len > 1 && all_equal {
        let _ = write!(s, "{len}{{{}}}", entries[0]);
    } else if len <= 1 || short_len == 0 || (len <= short_len && contiguous) {
        let _ = write!(s, "{len}(");
        for (k, e) in entries.iter().enumerate() {
            if k > 0 {
                s.push(' ');
            }
            s.push_str(e);
        }
        s.push(')');
    } else {
        let _ = write!(s, "\n{len}\n(");
        for e in entries {
            s.push('\n');
            s.push_str(e);
        }
        s.push_str("\n)\n");
    }
    s
}

/// `Field<Type>::writeEntry(keyword, os)` without the keyword and the closing `;`: `uniform x` or `nonuniform List<type> <list>`.
#[must_use]
pub fn field_entry_text(d: &Data, precision: usize) -> String {
    if d.is_uniform() {
        return format!("uniform {}", d.front_text(precision));
    }
    let mut s = format!("nonuniform List<{}> ", d.type_name());
    if d.is_empty() {
        s.push_str("0()");
    } else {
        let entries: Vec<String> = (0..d.len()).map(|i| d.element_text(i, precision)).collect();
        s.push_str(&write_list_text(&entries, true, false, 10));
    }
    s
}

/// `os.writeKeyword(keyword)` at `indent` levels (4 spaces each): the keyword padded to column 16 (at least one space).
fn keyword(indent: usize, key: &str) -> String {
    let mut s = String::new();
    for _ in 0..indent * 4 {
        s.push(' ');
    }
    s.push_str(key);
    let used = key.len();
    let pad = if used < 16 { 16 - used } else { 1 };
    for _ in 0..pad {
        s.push(' ');
    }
    s
}

/// The banner and the `FoamFile` header block of an object of `class` at `location` named `object`, then the blank line.
#[must_use]
pub fn header_text(class: &str, location: &str, object: &str) -> String {
    let mut s = String::from(BANNER);
    s.push_str("FoamFile\n{\n");
    let mut entry = |k: &str, v: &str| {
        s.push_str("    ");
        s.push_str(k);
        for _ in 0..(12usize.saturating_sub(k.len()).max(1)) {
            s.push(' ');
        }
        s.push_str(v);
        s.push_str(";\n");
    };
    entry("version", "2.0");
    entry("format", "ascii");
    entry("arch", &format!("\"{ARCH}\""));
    entry("class", class);
    entry("location", &format!("\"{location}\""));
    entry("object", object);
    s.push_str("}\n// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //\n\n");
    s
}

/// One patch of a `boundaryField`: its name, patch field type and the entries after `type` (`value`, `gradient`, ...) in write order.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchOut {
    /// Patch name.
    pub name: String,
    /// Patch field type (`zeroGradient`, `fixedValue`, ...): one of [`SUPPORTED_PATCH_TYPES`].
    pub ptype: String,
    /// `(keyword, values)` written as `Field::writeEntry` does.
    pub entries: Vec<(String, Data)>,
}

impl PatchOut {
    /// A patch with no entries after `type` (`empty`, `zeroGradient`, `slip`).
    #[must_use]
    pub fn plain(name: &str, ptype: &str) -> Self {
        Self { name: name.to_string(), ptype: ptype.to_string(), entries: Vec::new() }
    }

    /// A patch with one `value` entry (`fixedValue`, `calculated`).
    #[must_use]
    pub fn with_value(name: &str, ptype: &str, value: Data) -> Self {
        Self { name: name.to_string(), ptype: ptype.to_string(), entries: vec![("value".to_string(), value)] }
    }
}

/// A `GeometricField` file: `dimensions`, `internalField`, `boundaryField`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldFile {
    /// `class` of the header (`volScalarField`, `surfaceScalarField`, ...; `structure` for `alpha.structure`).
    pub class: String,
    /// `location` of the header, without quotes (`100/fluidRegion`).
    pub location: String,
    /// `object` (the file name).
    pub object: String,
    /// The seven exponents of `dimensionSet` (kg m s K mol A cd).
    pub dimensions: [i32; 7],
    /// `oriented oriented;` (a `surfaceScalarField` carries the orientation flag of `orientedType`).
    pub oriented: bool,
    /// `internalField`.
    pub internal: Data,
    /// `boundaryField`, in patch order.
    pub patches: Vec<PatchOut>,
}

fn dimensions_text(d: &[i32; 7]) -> String {
    let mut s = String::from("[");
    for (i, v) in d.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{v}");
    }
    s.push(']');
    s
}

/// The `entry` of a statement: `keyword<pad>value;\n` at `indent`; a value starting with a newline (a multi-line list) keeps the `;` on its own line.
fn statement(indent: usize, key: &str, value: &str) -> String {
    let mut s = keyword(indent, key);
    s.push_str(value);
    s.push_str(";\n");
    s
}

/// Write a `GeometricField` file (`GeometricField::writeData` + `regIOobject::writeObject`).
///
/// # Errors
/// [`FoamWriteError::PatchType`] for a patch type outside [`SUPPORTED_PATCH_TYPES`].
pub fn write_field_file(f: &FieldFile, opts: &WriteOptions) -> Result<String, FoamWriteError> {
    let p = opts.precision;
    let mut s = header_text(&f.class, &f.location, &f.object);
    s.push_str(&statement(0, "dimensions", &dimensions_text(&f.dimensions)));
    s.push('\n');
    if f.oriented {
        s.push_str(&statement(0, "oriented", "oriented"));
        s.push('\n');
    }
    s.push_str(&statement(0, "internalField", &field_entry_text(&f.internal, p)));
    s.push_str("\nboundaryField\n{\n");
    for patch in &f.patches {
        if !SUPPORTED_PATCH_TYPES.contains(&patch.ptype.as_str()) {
            return Err(FoamWriteError::PatchType(patch.ptype.clone()));
        }
        let _ = write!(s, "    {}\n    {{\n", patch.name);
        s.push_str(&statement(2, "type", &patch.ptype));
        for (k, d) in &patch.entries {
            s.push_str(&statement(2, k, &field_entry_text(d, p)));
        }
        s.push_str("    }\n");
    }
    s.push_str("}\n");
    s.push_str(FOOTER);
    Ok(s)
}

/// A bare list object (`vectorField` of `polyMesh/points`, `scalarFieldField` of `Trad.<model>`): the header and `os << list`.
///
/// `fields` is the list of lists for a field-of-fields (`entries.len() = fields.len()`), or one list for a plain field.
#[must_use]
pub fn write_list_file(class: &str, location: &str, object: &str, list: &ListObject, opts: &WriteOptions) -> String {
    let p = opts.precision;
    let mut s = header_text(class, location, object);
    match list {
        ListObject::Plain(d) => {
            let entries: Vec<String> = (0..d.len()).map(|i| d.element_text(i, p)).collect();
            s.push_str(&write_list_text(&entries, true, d.len() > 1 && d.is_uniform(), 10));
        }
        ListObject::FieldField(fields) => {
            let inner: Vec<String> = fields
                .iter()
                .map(|d| {
                    let entries: Vec<String> = (0..d.len()).map(|i| d.element_text(i, p)).collect();
                    write_list_text(&entries, true, d.len() > 1 && d.is_uniform(), 10)
                })
                .collect();
            // a list of lists is not contiguous: one element per line (`writeList`, `is_contiguous_v` false)
            s.push_str(&write_list_text(&inner, false, false, 10));
        }
    }
    s.push_str(FOOTER);
    s
}

/// What a bare list object holds.
#[derive(Debug, Clone, PartialEq)]
pub enum ListObject {
    /// `vectorField`, `scalarField`, ...
    Plain(Data),
    /// `scalarFieldField`: one list per cell (`Trad.nuclearFuelPin`).
    FieldField(Vec<Data>),
}

/// A dictionary object (`uniform/time`, `uniform/reactorState`): header, then each entry `keyword<pad>value;` followed by a blank line.
///
/// `entries` are `(keyword, already-formatted value)`; the values of numbers come from [`format_g`].
#[must_use]
pub fn write_dictionary_file(location: &str, object: &str, entries: &[(String, String)]) -> String {
    let mut s = header_text("dictionary", location, object);
    for (k, v) in entries {
        s.push_str(&statement(0, k, v));
        s.push('\n');
    }
    // each entry already ended with its own blank line; the footer supplies the final two newlines of `writeEndDivider`
    if !entries.is_empty() {
        s.pop();
    }
    s.push_str(FOOTER);
    s
}

/// `Time::writeTimeDict` (`uniform/time` of time directory `time_name`): `value` (18 digits), `name`, `index`, `deltaT`, `deltaT0` (`writePrecision`).
#[must_use]
pub fn write_time_dict(time_value: f64, name: &str, index: usize, delta_t: f64, delta_t0: f64, opts: &WriteOptions) -> String {
    let entries = vec![
        ("value".to_string(), format_g(time_value, MAX_TIME_PRECISION)),
        ("name".to_string(), format!("\"{name}\"")),
        ("index".to_string(), index.to_string()),
        ("deltaT".to_string(), format_g(delta_t, opts.precision)),
        ("deltaT0".to_string(), format_g(delta_t0, opts.precision)),
    ];
    write_dictionary_file(&format!("{name}/uniform"), "time", &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_g_is_c_printf_g() {
        // values checked against `printf '%.17g'` / `'%.6g'`
        assert_eq!(format_g(100.0, 17), "100");
        assert_eq!(format_g(0.5, 17), "0.5");
        assert_eq!(format_g(600.688_101_125_361_87, 17), "600.68810112536187");
        assert_eq!(format_g(1.0e-5, 17), "1.0000000000000001e-05");
        assert_eq!(format_g(2.0e7, 17), "20000000");
        assert_eq!(format_g(1.0e17, 17), "1e+17");
        assert_eq!(format_g(1.0e16, 17), "10000000000000000");
        assert_eq!(format_g(0.0001, 6), "0.0001");
        assert_eq!(format_g(0.00001, 6), "1e-05");
        assert_eq!(format_g(1_234_565.0, 6), "1.23456e+06");
        assert_eq!(format_g(123_456_789.0, 6), "1.23457e+08");
        assert_eq!(format_g(-0.0, 17), "-0");
        assert_eq!(format_g(0.0, 17), "0");
        assert_eq!(format_g(-1.5e-300, 17), "-1.5000000000000001e-300");
        assert_eq!(format_g(-1.5e-300, 3), "-1.5e-300");
        assert_eq!(format_g(2.5, 1), "2");
        assert_eq!(format_g(f64::NAN, 17), "nan");
        assert_eq!(format_g(f64::NEG_INFINITY, 17), "-inf");
        assert_eq!(time_name(0.2, 6), "0.2");
        assert_eq!(time_name(1.0e-6, 6), "1e-06");
        assert_eq!(time_name(0.1 + 0.2, 6), "0.3");
        assert_eq!(time_name(1.0 / 3.0, 6), "0.333333");
    }

    #[test]
    fn lists_follow_write_list() {
        let sc = |v: &[f64]| Data::scalar(v.to_vec());
        assert_eq!(field_entry_text(&sc(&[3.0]), 17), "uniform 3");
        assert_eq!(field_entry_text(&sc(&[0.0, 0.0, -0.0]), 17), "uniform 0");
        assert_eq!(field_entry_text(&sc(&[1.0, 2.0]), 17), "nonuniform List<scalar> 2(1 2)");
        assert_eq!(field_entry_text(&sc(&[]), 17), "nonuniform List<scalar> 0()");
        let long: Vec<f64> = (0..11).map(f64::from).collect();
        assert_eq!(
            field_entry_text(&sc(&long), 17),
            "nonuniform List<scalar> \n11\n(\n0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n)\n"
        );
        let v = Data::vector(&[[0.0, 0.0, 1.0], [0.0, 1.0, 0.0]]);
        assert_eq!(field_entry_text(&v, 17), "nonuniform List<vector> 2((0 0 1) (0 1 0))");
        assert_eq!(field_entry_text(&Data::vector(&[[0.0, 0.0, 1.0]; 3]), 17), "uniform (0 0 1)");
        // a list of equal contiguous entries (not reached through a field entry) is `N{x}`
        assert_eq!(write_list_text(&["7".to_string(), "7".to_string()], true, true, 10), "2{7}");
    }

    #[test]
    fn a_field_file_has_the_upstream_layout() {
        let f = FieldFile {
            class: "volScalarField".to_string(),
            location: "100/fluidRegion".to_string(),
            object: "htc".to_string(),
            dimensions: [1, 0, -3, -1, 0, 0, 0],
            oriented: false,
            internal: Data::scalar(vec![1.5, 2.5]),
            patches: vec![PatchOut::plain("inlet", "zeroGradient"), PatchOut::plain("frontAndBack", "empty")],
        };
        let text = write_field_file(&f, &WriteOptions::precision_17()).unwrap();
        assert!(text.starts_with("/*--------------------------------*- C++ -*"));
        assert!(text.contains("    arch        \"LSB;label=32;scalar=64\";\n    class       volScalarField;\n    location    \"100/fluidRegion\";\n    object      htc;\n}\n"));
        assert!(text.contains("\ndimensions      [1 0 -3 -1 0 0 0];\n\ninternalField   nonuniform List<scalar> 2(1.5 2.5);\n\nboundaryField\n{\n    inlet\n    {\n        type            zeroGradient;\n    }\n"));
        assert!(text.ends_with("    }\n}\n\n\n// ************************************************************************* //\n"));
        let mut bad = f;
        bad.patches.push(PatchOut::plain("x", "codedFixedValue"));
        assert_eq!(write_field_file(&bad, &WriteOptions::precision_17()), Err(FoamWriteError::PatchType("codedFixedValue".to_string())));
        assert!(format!("{}", FoamWriteError::Shape("x".to_string())).contains("malformed"));
    }

    #[test]
    fn dictionaries_and_list_objects_have_the_upstream_layout() {
        let t = write_time_dict(100.0, "100", 476, 0.208_333_333_333_333_34, 0.208_421_052_631_585_72, &WriteOptions::precision_17());
        assert!(t.contains("    location    \"100/uniform\";\n    object      time;\n"));
        assert!(t.contains("\nvalue           100;\n\nname            \"100\";\n\nindex           476;\n\ndeltaT          0.20833333333333334;\n\ndeltaT0         0.20842105263158572;\n\n\n// ****"));
        let pts = write_list_file("vectorField", "100/neutroRegion/polyMesh", "points", &ListObject::Plain(Data::vector(&[[0.0, 0.0, 0.0]; 2])), &WriteOptions::precision_17());
        assert!(pts.contains("//\n\n2{(0 0 0)}"));
        let ff = write_list_file("scalarFieldField", "100/fluidRegion", "Trad.x", &ListObject::FieldField(vec![Data::scalar(vec![1.0, 2.0]); 2]), &WriteOptions::precision_17());
        assert!(ff.contains("//\n\n\n2\n(\n2(1 2)\n2(1 2)\n)\n\n\n// ***"));
        assert_eq!(Data::flat(9, vec![0.0; 18]).type_name(), "tensor");
        assert_eq!(Data::flat(6, vec![0.0; 6]).len(), 1);
    }
}
