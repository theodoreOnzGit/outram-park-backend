//! ESI (OpenFOAM.com) v2506 ASCII file format, as an OPTION beside the
//! crate's own writers (gh:#1363 row 7; maintainer: "ESI preferred").
//!
//! The default writers (`PolyMesh::write`, `write_vol_*_field`) keep their
//! form: shortest-round-trip numbers, an "OUTRAM PARK" banner, no `arch` or
//! `note` entries. This module adds the layout ESI v2506 itself writes, so a
//! file produced here is byte-identical to the one `blockMesh`/`icoFoam`
//! writes where the numbers agree:
//!
//! - the ESI banner (`Version:  2506`), parameterised by
//!   [`EsiWriteOptions::version`];
//! - `arch "LSB;label=32;scalar=64";` in the `FoamFile` header, and the
//!   `note "nPoints:..  nCells:..  nFaces:..  nInternalFaces:.."` entry in
//!   `owner` and `neighbour`;
//! - two blank lines after the separator and (list files) before the footer;
//! - scalars through C++ `ostream` default formatting (`%g`) at
//!   `writePrecision` (default 6): see [`format_g`];
//! - `inGroups 1(<type>);` in `boundary` for `wall` and the constraint types.
//!
//! Upstream: OpenFOAM-v2506 `IOobject::writeHeader`
//! (`src/OpenFOAM/db/IOobject/IOobjectWriteHeader.C`), `polyMesh::writeObject`
//! and `polyPatch::write`; the banner comes from `foamHeader`.
//!
//! The ESI banner is the format ESI writes. Using it does not make the
//! producer OpenFOAM: that is why it is an option, and why the default
//! banner keeps naming this project.

use std::path::Path;

use super::poly_mesh::PolyMesh;
use super::IoError;
use crate::mesh::PatchKind;

/// Options of the ESI writers.
#[derive(Debug, Clone, PartialEq)]
pub struct EsiWriteOptions {
    /// Text after `Version:` in the banner (`"2506"` for the polyMesh files
    /// and fields ESI v2506 writes; its `controlDict` template says `v2506`).
    pub version: String,
    /// `writePrecision` (significant digits); ESI's default is 6.
    pub precision: usize,
}

impl Default for EsiWriteOptions {
    fn default() -> Self {
        Self {
            version: "2506".to_string(),
            precision: 6,
        }
    }
}

impl EsiWriteOptions {
    /// Same options at another `writePrecision`.
    #[must_use]
    pub fn with_precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self
    }
}

/// C++ `ostream` default floating-point formatting (`%g`) with `precision`
/// significant digits: what ESI writes for every `scalar`.
///
/// Fixed notation when `-4 <= exponent < precision`, otherwise scientific
/// with at least two exponent digits (`1e-05`); trailing zeros are removed.
pub fn format_g(x: f64, precision: usize) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan" } else { "nan" }.to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    let p = precision.max(1);
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let sci = format!("{:.*e}", p - 1, x);
    let (mantissa, exp_text) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp_text.parse().unwrap_or(0);
    if exp < -4 || exp >= p as i32 {
        let m = strip_zeros(mantissa);
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{m}e{sign}{:02}", exp.abs())
    } else {
        let decimals = (p as i32 - 1 - exp).max(0) as usize;
        strip_zeros(&format!("{:.*}", decimals, x))
    }
}

/// Remove trailing zeros after a decimal point, and the point itself if bare.
fn strip_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// ESI banner for `version`.
pub fn esi_banner(version: &str) -> String {
    format!(
        "/*--------------------------------*- C++ -*----------------------------------*\\\n\
| =========                 |                                                 |\n\
| \\\\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox           |\n\
|  \\\\    /   O peration     | Version:  {version:<38}|\n\
|   \\\\  /    A nd           | Website:  www.openfoam.com                      |\n\
|    \\\\/     M anipulation  |                                                 |\n\
\\*---------------------------------------------------------------------------*/\n"
    )
}

const SEPARATOR: &str =
    "// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //";
const FOOTER: &str =
    "// ************************************************************************* //\n";

/// Banner + `FoamFile` header + separator, ending after the separator line.
///
/// `extra` is written between `arch` and `class` (the `note` entry), as a
/// ready-made `("note", "\"...\"")` pair.
pub fn esi_header(
    opts: &EsiWriteOptions,
    extra: &[(&str, String)],
    class: &str,
    location: &str,
    object: &str,
) -> String {
    let mut s = esi_banner(&opts.version);
    s.push_str("FoamFile\n{\n");
    s.push_str("    version     2.0;\n    format      ascii;\n");
    s.push_str("    arch        \"LSB;label=32;scalar=64\";\n");
    for (k, v) in extra {
        s.push_str(&format!("    {k:<12}{v};\n"));
    }
    s.push_str(&format!("    class       {class};\n"));
    s.push_str(&format!("    location    \"{location}\";\n"));
    s.push_str(&format!("    object      {object};\n"));
    s.push_str("}\n");
    s.push_str(SEPARATOR);
    s.push('\n');
    s
}

/// `inGroups` value ESI prints for a patch type (`None` for a plain patch).
fn in_group(kind: PatchKind) -> Option<&'static str> {
    match kind {
        PatchKind::Patch => None,
        PatchKind::Wall => Some("wall"),
        PatchKind::Symmetry => Some("symmetry"),
        PatchKind::Empty => Some("empty"),
        PatchKind::Wedge => Some("wedge"),
        PatchKind::Cyclic => Some("cyclic"),
        PatchKind::CyclicAmi => Some("cyclicAMI"),
        PatchKind::Processor => Some("processor"),
    }
}

fn type_word(kind: PatchKind) -> &'static str {
    in_group(kind).unwrap_or("patch")
}

/// Write `<time_dir>/uniform/time` as ESI's `Time::writeObject` does:
/// `value` at 18 significant digits, the time `name`, step `index`, and
/// `deltaT` / `deltaT0` at `opts.precision`.
///
/// # Errors
/// [`IoError`] if the directory or file cannot be written.
pub fn write_time_file_esi(
    time_dir: impl AsRef<Path>,
    name: &str,
    value: f64,
    index: usize,
    delta_t: f64,
    delta_t0: f64,
    opts: &EsiWriteOptions,
) -> Result<(), IoError> {
    let time_dir = time_dir.as_ref();
    let dir = time_dir.join("uniform");
    std::fs::create_dir_all(&dir).map_err(|e| IoError::io(dir.display().to_string(), e))?;
    let location = format!("{name}/uniform");
    let mut s = esi_header(opts, &[], "dictionary", &location, "time");
    s.push('\n');
    s.push_str(&format!("value           {};\n\n", format_g(value, 18)));
    s.push_str(&format!("name            \"{name}\";\n\n"));
    s.push_str(&format!("index           {index};\n\n"));
    s.push_str(&format!(
        "deltaT          {};\n\n",
        format_g(delta_t, opts.precision)
    ));
    s.push_str(&format!(
        "deltaT0         {};\n\n\n",
        format_g(delta_t0, opts.precision)
    ));
    s.push_str(FOOTER);
    let path = dir.join("time");
    std::fs::write(&path, s).map_err(|e| IoError::io(path.display().to_string(), e))
}

/// ESI's `UList::writeList` rule for a list of already-formatted `items`:
/// `Some(text)` when ESI writes it on one line, `None` for the long form.
///
/// - more than one element, contiguous data, all equal: `N{value}`;
/// - at most one element, or at most 10 contiguous elements: `N(a b c)`;
/// - otherwise the long form (one element per line).
///
/// `contiguous` is false for lists of lists (faces).
pub(crate) fn inline_list(items: &[String], contiguous: bool) -> Option<String> {
    let n = items.len();
    if n > 1 && contiguous && items.iter().all(|x| *x == items[0]) {
        return Some(format!("{n}{{{}}}", items[0]));
    }
    if n <= 1 || (n <= 10 && contiguous) {
        return Some(format!("{n}({})", items.join(" ")));
    }
    None
}

/// Header followed by the list as ESI lays out a top-level list file.
fn finish_list(mut header: String, items: &[String], contiguous: bool) -> String {
    match inline_list(items, contiguous) {
        Some(text) => header.push_str(&format!("\n{text}\n\n")),
        None => {
            header.push_str(&format!("\n\n{}\n(\n", items.len()));
            for it in items {
                header.push_str(it);
                header.push('\n');
            }
            header.push_str(")\n\n\n");
        }
    }
    header.push_str(FOOTER);
    header
}

/// Write a `topoSet` file (`cellSet`, `faceSet`, `pointSet`) as ESI's
/// `topoSet::writeData` does (gh:#1384): the ESI header (class `class`,
/// `location`, object `name`) and the labels **in the order given**, which
/// must be the iteration order of upstream's `labelHashSet` (see
/// `outram_foam_mesh::topo_set::LabelHashSet`). Up to 10 labels go on one line
/// as `N(a b c)`, more in the long form; keys are never written as `N{v}`.
///
/// # Errors
/// [`IoError`] if the directory or file cannot be written.
pub fn write_label_set_esi(
    dir: impl AsRef<Path>,
    class: &str,
    location: &str,
    name: &str,
    labels: &[usize],
    opts: &EsiWriteOptions,
) -> Result<(), IoError> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|e| IoError::io(dir.display().to_string(), e))?;
    let header = esi_header(opts, &[], class, location, name);
    let items: Vec<String> = labels.iter().map(|l| l.to_string()).collect();
    let s = finish_list(header, &items, true);
    let path = dir.join(name);
    std::fs::write(&path, s).map_err(|e| IoError::io(path.display().to_string(), e))
}

/// Write `<dir>/cellZones` as ESI's `blockMesh` does (the zone names are also
/// listed in a `meta` entry of the header). Nothing is written for no zones.
///
/// # Errors
/// [`IoError`] if the file cannot be written.
pub fn write_cell_zones_esi(
    dir: impl AsRef<Path>,
    location: &str,
    zones: &[(String, Vec<usize>)],
    opts: &EsiWriteOptions,
) -> Result<(), IoError> {
    if zones.is_empty() {
        return Ok(());
    }
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|e| IoError::io(dir.display().to_string(), e))?;
    let names: Vec<&str> = zones.iter().map(|(n, _)| n.as_str()).collect();
    let header = esi_header(opts, &[], "regIOobject", location, "cellZones");
    let meta = format!(
        "    object      cellZones;\n    meta\n    {{\n        names           {} ( {} );\n    }}\n}}\n",
        zones.len(),
        names.join(" ")
    );
    let mut s = header.replace("    object      cellZones;\n}\n", &meta);
    s.push_str(&format!("\n{}\n(", zones.len()));
    for (i, (name, cells)) in zones.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(&format!(
            "{name}\n{{\n    type            cellZone;\n    cellLabels      List<label> "
        ));
        let items: Vec<String> = cells.iter().map(ToString::to_string).collect();
        match inline_list(&items, true) {
            Some(t) => s.push_str(&format!("{t};\n")),
            None => {
                s.push_str(&format!("\n{}\n(\n", items.len()));
                for it in &items {
                    s.push_str(it);
                    s.push('\n');
                }
                s.push_str(")\n;\n");
            }
        }
        s.push('}');
    }
    s.push_str("\n)\n\n");
    s.push_str(FOOTER);
    let path = dir.join("cellZones");
    std::fs::write(&path, s).map_err(|e| IoError::io(path.display().to_string(), e))
}

impl PolyMesh {
    /// Write `points`, `faces`, `owner`, `neighbour`, `boundary` into `dir`
    /// in ESI v2506's layout (see the module doc). `location` is the string
    /// of the header's `location` entry, e.g. `"constant/polyMesh"` or
    /// `"1/polyMesh"`.
    ///
    /// # Errors
    /// [`IoError`] if a file cannot be written.
    pub fn write_esi(
        &self,
        dir: impl AsRef<Path>,
        location: &str,
        opts: &EsiWriteOptions,
    ) -> Result<(), IoError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir).map_err(|e| IoError::io(dir.display().to_string(), e))?;
        let put = |name: &str, text: String| -> Result<(), IoError> {
            let p = dir.join(name);
            std::fs::write(&p, text).map_err(|e| IoError::io(p.display().to_string(), e))
        };
        put("points", self.points_esi(location, opts))?;
        put("faces", self.faces_esi(location, opts))?;
        put("owner", self.owner_esi(location, opts))?;
        put("neighbour", self.neighbour_esi(location, opts))?;
        put("boundary", self.boundary_esi(location, opts))?;
        Ok(())
    }

    fn mesh_note(&self) -> String {
        format!(
            "\"nPoints:{}  nCells:{}  nFaces:{}  nInternalFaces:{}\"",
            self.points.len(),
            self.n_cells,
            self.faces.len(),
            self.n_internal_faces
        )
    }

    fn points_esi(&self, location: &str, opts: &EsiWriteOptions) -> String {
        let header = esi_header(opts, &[], "vectorField", location, "points");
        let items: Vec<String> = self
            .points
            .iter()
            .map(|p| {
                format!(
                    "({} {} {})",
                    format_g(p.x, opts.precision),
                    format_g(p.y, opts.precision),
                    format_g(p.z, opts.precision)
                )
            })
            .collect();
        finish_list(header, &items, true)
    }

    fn faces_esi(&self, location: &str, opts: &EsiWriteOptions) -> String {
        let header = esi_header(opts, &[], "faceList", location, "faces");
        let items: Vec<String> = self
            .faces
            .iter()
            .map(|f| {
                let verts: Vec<String> = f.verts.iter().map(ToString::to_string).collect();
                format!("{}({})", f.verts.len(), verts.join(" "))
            })
            .collect();
        // A face is not contiguous data: only a list of at most one is inline.
        finish_list(header, &items, false)
    }

    fn owner_esi(&self, location: &str, opts: &EsiWriteOptions) -> String {
        let note = [("note", self.mesh_note())];
        let header = esi_header(opts, &note, "labelList", location, "owner");
        let items: Vec<String> = self.faces.iter().map(|f| f.owner.to_string()).collect();
        finish_list(header, &items, true)
    }

    fn neighbour_esi(&self, location: &str, opts: &EsiWriteOptions) -> String {
        let note = [("note", self.mesh_note())];
        let header = esi_header(opts, &note, "labelList", location, "neighbour");
        let items: Vec<String> = self
            .faces
            .iter()
            .take(self.n_internal_faces)
            .map(|f| f.neighbour.unwrap_or(0).to_string())
            .collect();
        finish_list(header, &items, true)
    }

    fn boundary_esi(&self, location: &str, opts: &EsiWriteOptions) -> String {
        let mut s = esi_header(opts, &[], "polyBoundaryMesh", location, "boundary");
        s.push_str(&format!("\n{}\n(\n", self.patches.len()));
        for p in &self.patches {
            s.push_str(&format!("    {}\n    {{\n", p.name));
            s.push_str(&format!("        type            {};\n", type_word(p.kind)));
            if let Some(g) = in_group(p.kind) {
                s.push_str(&format!("        inGroups        1({g});\n"));
            }
            s.push_str(&format!("        nFaces          {};\n", p.size));
            s.push_str(&format!("        startFace       {};\n", p.start));
            s.push_str("    }\n");
        }
        s.push_str(")\n\n");
        s.push_str(FOOTER);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_g_matches_c_printf_g() {
        let g = |x: f64, p: usize| format_g(x, p);
        assert_eq!(g(0.0, 6), "0");
        assert_eq!(g(1.0, 6), "1");
        assert_eq!(g(0.005, 6), "0.005");
        assert_eq!(g(0.1 * 0.35, 6), "0.035");
        assert_eq!(g(4.299_31e-6, 6), "4.29931e-06");
        assert_eq!(g(123_456.7, 6), "123457");
        assert_eq!(g(1_234_567.0, 6), "1.23457e+06");
        assert_eq!(g(0.000_123_456_7, 6), "0.000123457");
        assert_eq!(g(0.000_012_345_67, 6), "1.23457e-05");
        assert_eq!(g(-2.5, 6), "-2.5");
        assert_eq!(g(1.0 / 3.0, 17), "0.33333333333333331");
        assert_eq!(g(100.0, 3), "100");
        assert_eq!(g(1000.0, 3), "1e+03");
        assert_eq!(g(f64::INFINITY, 6), "inf");
    }

    /// ESI `UList::writeList`: uniform `N{v}`, up to 10 contiguous inline,
    /// longer lists in the long form, lists of lists inline only up to one.
    #[test]
    fn inline_list_follows_the_esi_short_list_rule() {
        let s = |v: &[&str]| v.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(inline_list(&s(&[]), true).as_deref(), Some("0()"));
        assert_eq!(inline_list(&s(&["7"]), true).as_deref(), Some("1(7)"));
        assert_eq!(inline_list(&s(&["3", "3", "3"]), true).as_deref(), Some("3{3}"));
        assert_eq!(inline_list(&s(&["1", "2", "3"]), true).as_deref(), Some("3(1 2 3)"));
        let eleven: Vec<String> = (0..11).map(|i| i.to_string()).collect();
        assert_eq!(inline_list(&eleven, true), None);
        assert_eq!(inline_list(&s(&["4(0 1 2 3)", "4(1 2 3 4)"]), false), None);
        assert_eq!(inline_list(&s(&["4(0 1 2 3)"]), false).as_deref(), Some("1(4(0 1 2 3))"));
    }

    #[test]
    fn banner_is_79_wide_and_names_the_version() {
        let b = esi_banner("2506");
        for line in b.lines() {
            assert_eq!(line.len(), 79, "{line}");
        }
        assert!(b.contains("Version:  2506 "));
    }
}
