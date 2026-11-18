// SPDX-License-Identifier: GPL-3.0

//! **The LEU-COMP-THERM-008 lattice model** (ICSBEP; the B&W critical
//! lattices, a 2.459 w/o UO2 rod array in borated water).
//!
//! The committed `mit-crpg/benchmarks` OpenMC cards for cases 1, 2 and 8
//! (`verification_and_validation/icsbep/leu-comp-therm-008/`, Paul Romano, MIT
//! licence), parsed into materials and a CSG lattice, plus the geometry
//! self-check and LCT-008's pin in a reflective square cell. `no_std` +
//! `alloc`: the cards are compiled in, nothing here reads a file, prints or
//! loads nuclear data.
//!
//! **Moved 2026-11-17** from `examples/common/lct008_model.rs` and
//! `examples/common/lct008_pin_cell.rs` (maintainer: the validation cases live
//! in the library, especially what can be `no_std`). The arithmetic is
//! unchanged. Two things changed shape so the module can be `no_std`:
//!
//! - the case is an argument ([`Case`]) instead of a process-wide `OnceLock`,
//!   and the parsed lattice bookkeeping ([`Model`]) is returned by
//!   [`build_geometry_and_model`] instead of stashed in a static;
//! - [`check_geometry`] returns a [`GeometryCheck`] instead of printing, and
//!   the omission report is [`omissions_report`], a list of lines.
//!
//! The std glue (the `--case` switch, the printed lines) stays in
//! `examples/common/lct008_model.rs`, which re-exports this module, so every
//! example and `dhoby-ghaut`'s web rung keep running one model.
//!
//! The provenance, the approximations and every recorded result stay in
//! `examples/lct008_keff.rs`'s module documentation, which is the authority
//! for the model.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::collections::BTreeMap;
use core::str::FromStr;
#[allow(unused_imports)]
use petir::mathf::RealMath;

use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use crate::geometry::geometry::Geometry;
use crate::geometry::lattice::{Lattice, RectLattice};
use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use crate::geometry::universe::Universe;
use crate::material::material::{Material, NuclideComponent};
use crate::material::material_sab::{assign_material_sab, MaterialSab};
use crate::material::nuclide::Nuclide;
use crate::material::thermal::ThermalScattering;
use crate::physics::transport_csg::SourceBox;

/// The benchmark is at room temperature and the model gives no `<temperature>`,
/// so OpenMC's default applies. 293.6 K is also the tabulated temperature of the
/// `H(H2O)` law, so no thermal interpolation is involved.
pub const TEMP_K: f64 = 293.6;

pub const MATERIALS_XML_CASE1: &str =
    include_str!("../../verification_and_validation/icsbep/leu-comp-therm-008/materials.xml");
pub const GEOMETRY_XML_CASE1: &str =
    include_str!("../../verification_and_validation/icsbep/leu-comp-therm-008/geometry.xml");
pub const MATERIALS_XML_CASE2: &str = include_str!(
    "../../verification_and_validation/icsbep/leu-comp-therm-008/case-2/materials.xml"
);
pub const GEOMETRY_XML_CASE2: &str =
    include_str!("../../verification_and_validation/icsbep/leu-comp-therm-008/case-2/geometry.xml");
pub const MATERIALS_XML_CASE8: &str = include_str!(
    "../../verification_and_validation/icsbep/leu-comp-therm-008/case-8/materials.xml"
);
pub const GEOMETRY_XML_CASE8: &str =
    include_str!("../../verification_and_validation/icsbep/leu-comp-therm-008/case-8/geometry.xml");

/// The committed case (the examples' `--case`).
///
/// Every case of LEU-COMP-THERM-008 is **independently critical**, and they
/// differ in how the poison is supplied: case 1 carries 1511 ppm of *soluble*
/// boron and no poison rods; case 8 carries 794 ppm and 144 *lumped* pyrex
/// burnable-poison rods. Same lattice, same pitch, same fuel. That makes the
/// pair a **differential measurement needing no new reference**: an error in
/// thermal absorption tracks the soluble-boron worth and its distribution, while
/// an error in epithermal resonance escape does not, because `p` is set by the
/// lattice and the lattice is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Case 1: 1511 ppm soluble boron, no poison rods.
    One,
    /// Case 2.
    Two,
    /// Case 8: 794 ppm soluble boron and 144 pyrex burnable-poison rods.
    Eight,
}

impl Case {
    /// The case for a `--case` number; any number other than 2 or 8 is case
    /// 1, as the examples' switch always read it.
    pub fn from_number(n: u32) -> Self {
        match n {
            2 => Case::Two,
            8 => Case::Eight,
            _ => Case::One,
        }
    }

    /// The case's `materials.xml`.
    pub fn materials_xml(self) -> &'static str {
        match self {
            Case::Two => MATERIALS_XML_CASE2,
            Case::Eight => MATERIALS_XML_CASE8,
            Case::One => MATERIALS_XML_CASE1,
        }
    }

    /// The case's `geometry.xml`.
    pub fn geometry_xml(self) -> &'static str {
        match self {
            Case::Two => GEOMETRY_XML_CASE2,
            Case::Eight => GEOMETRY_XML_CASE8,
            Case::One => GEOMETRY_XML_CASE1,
        }
    }
}

/// Pin universes, transcribed **by hand** off the committed `geometry.xml` — two
/// or three cells each, which is inside the size a reviewer can check by eye
/// (GitHub #184's rule). Each entry is `(universe id, [(outer radius, material
/// id)], material id outside them all)`.
pub const PIN_SHELLS: &[(i32, &[(f64, i32)], i32)] = &[
    // all-water pin cell
    (1, &[], 1),
    // fuel rod: UO2 to 0.514858, Al-6061 clad to 0.602996, water outside
    (2, &[(0.514_858, 2), (0.602_996, 3)], 1),
    // pyrex burnable-poison rod (case 8): glass to 0.585, water outside
    (3, &[(0.585_000, 4)], 1),
];

/// Nuclides this environment has an ENDF/B-VIII.0 tape for, by the OpenMC name
/// the model uses. Anything in the model and not in this table is omitted, and
/// the omission is reported and bounded (see the module docs).
/// **The default tape set: every nuclide the OpenMC material cards name.**
///
/// Correct physics is the DEFAULT, not an opt-in (`develop`, 2026-09-20). The
/// clad's trace alloying elements are part of the model, so they are loaded
/// unless someone deliberately asks for less with `--cheap-nuclides`.
///
/// # Cost
///
/// Each tape is resonance-reconstructed and Doppler-broadened on device
/// (RECONR + BROADR); the iron and chromium evaluations alone are 8-24 MB of
/// ENDF text apiece, and reconstruction dominates the run before a single
/// neutron moves. [`TAPES_CHEAP`] exists for iterating on geometry, tallies or
/// statistics, where the clad's trace elements change nothing being looked at.
///
/// # ~~Known ACE differences~~ Fixed (gh:#875)
///
/// ~~The Rust ACER's tables for 12 of the clad's trace nuclides (Na, Ti, Cr,
/// Fe-57/58, Cu-65, Zn) differ from NJOY2016's: 24 of 74 tables at
/// 293.6 K and 0 K.~~ **CORRECTED 2026-10-15:** all 74 tables of
/// `nuclides.tsv` (37 nuclides, 293.6 K and 0 K) are NJOY2016's in every
/// word. Four of the six fixes were upstream of ACER (RECONR's `facphi` guard
/// and resonance-grid end; PURR's grid and parameter law), so they reach
/// route 4 and the browser demo too, on Ti-48, Zn and Fe-58. The
/// `lct008-full` rows of 2026-10-14 were taken before the fixes and have not
/// been re-measured.
pub const TAPES: &[(&str, &str)] = &[
    // Everything in the cheap tier ...
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    // ... plus the Al-6061 trace alloying elements, added 2026-09-20 from the
    // local ENDF/B-VIII.0 library. Before this they were absent and the model
    // ran WITHOUT them.
    ("B11", "n-005_B_011-ENDF8.0.endf"),
    ("Na23", "n-011_Na_023-ENDF8.0.endf"),
    ("Mg24", "n-012_Mg_024-ENDF8.0.endf"),
    ("Mg25", "n-012_Mg_025-ENDF8.0.endf"),
    ("Mg26", "n-012_Mg_026-ENDF8.0.endf"),
    ("Ti46", "n-022_Ti_046-ENDF8.0.endf"),
    ("Ti47", "n-022_Ti_047-ENDF8.0.endf"),
    ("Ti48", "n-022_Ti_048-ENDF8.0.endf"),
    ("Ti49", "n-022_Ti_049-ENDF8.0.endf"),
    ("Ti50", "n-022_Ti_050-ENDF8.0.endf"),
    ("Cr50", "n-024_Cr_050-ENDF8.0.endf"),
    ("Cr52", "n-024_Cr_052-ENDF8.0.endf"),
    ("Cr53", "n-024_Cr_053-ENDF8.0.endf"),
    ("Cr54", "n-024_Cr_054-ENDF8.0.endf"),
    ("Fe54", "n-026_Fe_054-ENDF8.0.endf"),
    ("Fe56", "n-026_Fe_056-ENDF8.0.endf"),
    ("Fe57", "n-026_Fe_057-ENDF8.0.endf"),
    ("Fe58", "n-026_Fe_058-ENDF8.0.endf"),
    ("Cu63", "n-029_Cu_063-ENDF8.0.endf"),
    ("Cu65", "n-029_Cu_065-ENDF8.0.endf"),
    ("Zn64", "n-030_Zn_064-ENDF8.0.endf"),
    ("Zn66", "n-030_Zn_066-ENDF8.0.endf"),
    ("Zn67", "n-030_Zn_067-ENDF8.0.endf"),
    ("Zn68", "n-030_Zn_068-ENDF8.0.endf"),
    ("Zn70", "n-030_Zn_070-ENDF8.0.endf"),
];

/// The **cheap** subset — fuel, moderator, soluble-boron poison, and the three
/// clad nuclides the clad cannot do without. Selected with `--cheap-nuclides`.
///
/// **A smoke test only, since 2026-10-14 (maintainer direction): the
/// 11-nuclide model is retired.** It exercises the identical transport path
/// at a fraction of the reconstruction cost, so it serves to check that the
/// code runs (the acceptance check's LCT-008 case) and to iterate on
/// geometry, tallies or statistics. No result on it is quoted as the
/// lattice's `k`: the model it describes is incomplete (the 24 dropped
/// nuclides are worth −75 ± 14 pcm, gh:#533), and correct physics is the
/// default here.
pub const TAPES_CHEAP: &[(&str, &str)] = &[
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    // ── Clad, CHEAP tier: the three the clad cannot do without ───────────
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
];

/// Every `<tag …>` element in `xml`, as `(attributes, body)`. `body` is empty
/// for a self-closing tag. Matching requires a delimiter after the tag name, so
/// `<material` does not match `<materials`.
pub fn elements<'a>(xml: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(p) = xml[i..].find(&open) {
        let start = i + p;
        let after = start + open.len();
        let next = xml[after..].chars().next().unwrap_or('>');
        if !(next.is_whitespace() || next == '>' || next == '/') {
            i = after;
            continue;
        }
        let gt = start
            + xml[start..]
                .find('>')
                .unwrap_or_else(|| panic!("unterminated <{tag}>"));
        let raw = &xml[after..gt];
        let self_closing = raw.trim_end().ends_with('/');
        let attrs = raw.trim_end().trim_end_matches('/');
        let body = if self_closing {
            ""
        } else {
            let rest = &xml[gt + 1..];
            let e = rest
                .find(&close)
                .unwrap_or_else(|| panic!("unterminated <{tag}> element"));
            &rest[..e]
        };
        out.push((attrs, body));
        i = gt + 1;
    }
    out
}

/// The value of attribute `name` in an attribute string, or `None`.
pub fn attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut i = 0usize;
    while let Some(p) = attrs[i..].find(name) {
        let s = i + p;
        let ok_before = s == 0 || attrs[..s].ends_with(char::is_whitespace);
        let rest = attrs[s + name.len()..].trim_start();
        if ok_before && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let end = v[1..].find(q)? + 1;
                return Some(&v[1..end]);
            }
        }
        i = s + name.len();
    }
    None
}

/// Strip `<!-- … -->`. The geometry file comments out a surface and annotates
/// every other one; a comment body must never be read as markup.
pub fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(p) = rest.find("<!--") {
        out.push_str(&rest[..p]);
        match rest[p..].find("-->") {
            Some(e) => rest = &rest[p + e + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// `s`, trimmed and parsed; panics naming `what` when it does not parse.
pub fn num<T: FromStr>(s: &str, what: &str) -> T {
    s.trim()
        .parse()
        .unwrap_or_else(|_| panic!("{what}: cannot parse {s:?}"))
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

/// One `<material>` card: id, name, `(nuclide, atoms/b-cm)` and the `<sab>`
/// table it names, if any.
pub struct MaterialSpec {
    pub id: i32,
    pub name: String,
    pub nuclides: Vec<(String, f64)>,
    pub sab: Option<String>,
}

/// Every `<material>` card of `xml`, in file order.
pub fn parse_materials(xml: &str) -> Vec<MaterialSpec> {
    let xml = strip_comments(xml);
    elements(&xml, "material")
        .into_iter()
        .map(|(a, body)| MaterialSpec {
            id: num(attr(a, "id").expect("material id"), "material id"),
            name: attr(a, "name").unwrap_or("").to_string(),
            nuclides: elements(body, "nuclide")
                .into_iter()
                .map(|(na, _)| {
                    (
                        attr(na, "name").expect("nuclide name").to_string(),
                        num(attr(na, "ao").expect("nuclide ao"), "nuclide ao"),
                    )
                })
                .collect(),
            sab: elements(body, "sab")
                .first()
                .and_then(|(sa, _)| attr(sa, "name"))
                .map(str::to_string),
        })
        .collect()
}

/// Build the transport materials, in the model's own order. Returns the
/// materials and the index of the clad. With `bound_omission`, the omitted
/// clad density is re-added as Mn-55 (see the module docs).
pub fn build_materials(
    spec: &[MaterialSpec],
    slots: &BTreeMap<String, usize>,
    omitted: &BTreeMap<String, f64>,
    bound_omission: bool,
) -> (Vec<Material>, usize) {
    let mut clad_idx = 0usize;
    let materials = spec
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut components: Vec<NuclideComponent> = m
                .nuclides
                .iter()
                .filter_map(|(name, ao)| {
                    slots.get(name).map(|&nuclide_idx| NuclideComponent {
                        nuclide_idx,
                        atom_density: *ao,
                    })
                })
                .collect();
            if m.name.contains("Aluminum") || m.name.contains("Cladding") {
                clad_idx = i;
                if bound_omission {
                    let extra: f64 = m
                        .nuclides
                        .iter()
                        .filter(|(n, _)| omitted.contains_key(n))
                        .map(|(_, ao)| *ao)
                        .sum();
                    let mn = slots["Mn55"];
                    for c in components.iter_mut() {
                        if c.nuclide_idx == mn {
                            c.atom_density += extra;
                        }
                    }
                }
            }
            Material {
                id: m.id,
                name: m.name.clone(),
                temperature: TEMP_K,
                components,
            }
        })
        .collect();
    (materials, clad_idx)
}

/// Attach `H(H2O)` per material, as the model's own `<sab>` cards say
/// (GitHub #462, #1581): a material whose card names `c_H_in_H2O` and that
/// holds H-1 gets a bound copy of H-1; every other material keeps the free
/// entry. `nuclides` must hold H-1 **free** (the loaders no longer bind it).
/// The bound copy is appended to the returned array. With no table or no H-1
/// loaded, both inputs come back unchanged.
///
/// Audited 2026-11-14 on the three committed cases: only the water carries H-1
/// and only the water has a `<sab>` card, so no free material references the
/// bound copy.
pub fn assign_spec_sab(
    spec: &[MaterialSpec],
    materials: &[Material],
    nuclides: &[Nuclide],
    slots: &BTreeMap<String, usize>,
    sab: Option<&ThermalScattering>,
) -> (Vec<Material>, Vec<Nuclide>) {
    let (Some(table), Some(&h1)) = (sab, slots.get("H1")) else {
        return (materials.to_vec(), nuclides.to_vec());
    };
    let entries: Vec<MaterialSab> = spec
        .iter()
        .enumerate()
        .filter(|(i, m)| {
            m.sab.as_deref() == Some("c_H_in_H2O")
                && materials[*i].components.iter().any(|c| c.nuclide_idx == h1)
        })
        .map(|(i, _)| MaterialSab { material: i, nuclide: h1, table: 0, fraction: 1.0 })
        .collect();
    assign_material_sab(materials, nuclides, core::slice::from_ref(table), &entries)
        .expect("per-material H(H2O)")
}

/// What [`build_materials`] leaves out: one line per material that loses
/// atoms, then the total. One line saying nothing was omitted when `omitted`
/// is empty. The examples print these to stderr.
pub fn omissions_report(spec: &[MaterialSpec], omitted: &BTreeMap<String, f64>) -> Vec<String> {
    if omitted.is_empty() {
        return vec!["  every nuclide in the model has a tape — nothing omitted.".to_string()];
    }
    let mut lines = Vec::new();
    let names: Vec<&str> = omitted.keys().map(String::as_str).collect();
    let total: f64 = omitted.values().sum();
    for m in spec {
        let sum: f64 = m.nuclides.iter().map(|(_, ao)| *ao).sum();
        let miss: f64 = m
            .nuclides
            .iter()
            .filter(|(n, _)| omitted.contains_key(n))
            .map(|(_, ao)| *ao)
            .sum();
        if miss > 0.0 {
            lines.push(format!(
                "  material {} \"{}\": omitted {:.3e} of {:.3e} /b·cm ({:.2} % of its atoms)",
                m.id,
                m.name,
                miss,
                sum,
                100.0 * miss / sum
            ));
        }
    }
    lines.push(format!(
        "  omitted (not in the active nuclide tier, or no tape): {} — {:.3e} /b·cm in total",
        names.join(", "),
        total
    ));
    lines
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Core bounding cylinder [cm] — surface 30 in the model, vacuum.
pub const R_CORE: f64 = 76.200;
/// Core axial extent [cm] — surfaces 10 and 20, both vacuum.
pub const Z_LO: f64 = -81.662;
/// Top of the core [cm] (surface 20).
pub const Z_HI: f64 = 81.662;

/// The parsed model, kept so the self-check can re-derive lattice lookups by
/// its own arithmetic instead of trusting `Geometry::locate`.
#[derive(Debug, Clone)]
pub struct Model {
    /// Core lattice, and each assembly lattice, by id.
    lattices: BTreeMap<i32, RectLattice>,
    /// Universe id → the lattice id filling it, for the assembly wrapper cells.
    wrapper: BTreeMap<i32, i32>,
    /// Universe id → the *universe* id filling it, for the water-tile wrapper.
    uni_fill: BTreeMap<i32, i32>,
    /// Universe id at each universe slot, in `Geometry.universes` order.
    uni_ids: Vec<i32>,
}

/// The case's CSG: surfaces, the nested lattices and the cells, in card
/// order, with the lattice bookkeeping [`check_geometry`] needs.
/// `reverse_rows` reads `<universes>` top row first (OpenMC's order, the
/// canonical build); `false` builds the y-mirror the self-check compares
/// against.
pub fn build_geometry_and_model(case: Case, materials: &[Material], reverse_rows: bool) -> (Geometry, Model) {
    let xml = strip_comments(case.geometry_xml());

    // --- surfaces -------------------------------------------------------
    let mut surf_index: BTreeMap<i32, usize> = BTreeMap::new();
    let mut surfaces: Vec<SurfaceKind> = Vec::new();
    for (a, _) in elements(&xml, "surface") {
        let id: i32 = num(attr(a, "id").expect("surface id"), "surface id");
        let ty = attr(a, "type").expect("surface type");
        let c: Vec<f64> = attr(a, "coeffs")
            .expect("surface coeffs")
            .split_whitespace()
            .map(|t| num(t, "surface coeff"))
            .collect();
        let bc = match attr(a, "boundary") {
            Some("vacuum") => BoundaryType::Vacuum,
            Some("reflective") => BoundaryType::Reflective,
            Some(other) => panic!("unhandled boundary {other:?} on surface {id}"),
            None => BoundaryType::Transmissive,
        };
        let s = match ty {
            "z-cylinder" => SurfaceKind::ZCylinder(ZCylinder {
                x0: c[0],
                y0: c[1],
                r: c[2],
                bc,
            }),
            "z-plane" => SurfaceKind::ZPlane(ZPlane { z0: c[0], bc }),
            "sphere" => SurfaceKind::Sphere(Sphere {
                x0: c[0],
                y0: c[1],
                z0: c[2],
                r: c[3],
                bc,
            }),
            other => panic!("unhandled surface type {other:?} on surface {id}"),
        };
        surf_index.insert(id, surfaces.len());
        surfaces.push(s);
    }

    // --- lattices -------------------------------------------------------
    // Universe ids are collected first: a lattice's `<universes>` block names
    // them, and so does every cell's `universe=` attribute.
    let lat_blocks = elements(&xml, "lattice");
    let cell_blocks = elements(&xml, "cell");
    let mut uni_ids: Vec<i32> = Vec::new();
    for (a, _) in &cell_blocks {
        let u: i32 = num(attr(a, "universe").expect("cell universe"), "cell universe");
        if !uni_ids.contains(&u) {
            uni_ids.push(u);
        }
    }
    uni_ids.sort_unstable();
    let uni_index: BTreeMap<i32, usize> =
        uni_ids.iter().enumerate().map(|(i, &u)| (u, i)).collect();

    let mut lat_index: BTreeMap<i32, usize> = BTreeMap::new();
    let mut lattices: Vec<Lattice> = Vec::new();
    let mut model_lattices: BTreeMap<i32, RectLattice> = BTreeMap::new();
    for (a, body) in &lat_blocks {
        let id: i32 = num(attr(a, "id").expect("lattice id"), "lattice id");
        let dim: Vec<usize> = attr(a, "dimension")
            .expect("lattice dimension")
            .split_whitespace()
            .map(|t| num(t, "dimension"))
            .collect();
        assert_eq!(dim.len(), 2, "lattice {id} is not 2-D; not handled here");
        let (nx, ny) = (dim[0], dim[1]);
        let ll: Vec<f64> = block(body, "lower_left");
        let pitch: Vec<f64> = block(body, "pitch");
        let rows: Vec<i32> = block(body, "universes");
        assert_eq!(
            rows.len(),
            nx * ny,
            "lattice {id}: {} entries for a {nx}×{ny} grid",
            rows.len()
        );

        // OpenMC writes lattice rows from the TOP down — the first row is the
        // one at maximum y (`openmc/lattice.py`, `Lattice.universes` docs, and
        // the XML writer's row ordering). This crate's flat index runs with iy
        // increasing along +y. Reversing the rows here is the whole of that
        // conversion, and getting it wrong mirrors the core in y — a defect
        // that produces a perfectly plausible wrong k. `check_geometry` tests
        // the convention against the data itself.
        // Every lattice in this model is centred on its own frame — asserted
        // below — which is what makes the reversal a pure mirror.
        assert!(
            (ll[1] + 0.5 * ny as f64 * pitch[1]).abs() < 1.0e-9,
            "lattice {id} is not centred in y; the mirror argument in \
             `check_geometry` does not apply"
        );
        let mut universes = vec![0usize; nx * ny];
        for row in 0..ny {
            let iy = if reverse_rows { ny - 1 - row } else { row };
            for ix in 0..nx {
                let uid = rows[row * nx + ix];
                universes[nx * iy + ix] = *uni_index
                    .get(&uid)
                    .unwrap_or_else(|| panic!("lattice {id} names unknown universe {uid}"));
            }
        }
        let rect = RectLattice {
            id,
            n: [nx, ny, 1],
            lower_left: Position::new(ll[0], ll[1], 0.0),
            pitch: [pitch[0], pitch[1], 1.0],
            universes,
            outer: None,
        };
        model_lattices.insert(id, rect.clone());
        lat_index.insert(id, lattices.len());
        lattices.push(Lattice::Rect(rect));
    }

    // --- cells ----------------------------------------------------------
    let mat_index: BTreeMap<i32, usize> = materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id, i))
        .collect();
    let mut cells: Vec<Cell> = Vec::new();
    let mut universes: Vec<Universe> = uni_ids
        .iter()
        .map(|&id| Universe {
            id,
            cell_indices: Vec::new(),
        })
        .collect();
    let mut wrapper: BTreeMap<i32, i32> = BTreeMap::new();
    let mut uni_fill: BTreeMap<i32, i32> = BTreeMap::new();
    for (a, _) in &cell_blocks {
        let id: i32 = num(attr(a, "id").expect("cell id"), "cell id");
        let uid: i32 = num(attr(a, "universe").expect("cell universe"), "cell universe");
        let region = parse_region(attr(a, "region").expect("cell region"), &surf_index, id);
        let fill = match (attr(a, "material"), attr(a, "fill")) {
            (Some(m), None) => {
                let mid: i32 = num(m, "cell material");
                CellFill::Material(
                    *mat_index
                        .get(&mid)
                        .unwrap_or_else(|| panic!("cell {id} names unknown material {mid}")),
                )
            }
            (None, Some(f)) => {
                let fid: i32 = num(f, "cell fill");
                // A fill id names a lattice if one has that id, else a universe.
                // The two id spaces are disjoint in this model, and that is
                // asserted rather than assumed.
                match (lat_index.get(&fid), uni_index.get(&fid)) {
                    (Some(_), Some(_)) => {
                        panic!("cell {id}: fill {fid} is ambiguous — both a lattice and a universe")
                    }
                    (Some(&l), None) => {
                        wrapper.insert(uid, fid);
                        CellFill::Lattice(l)
                    }
                    (None, Some(&u)) => {
                        uni_fill.insert(uid, fid);
                        CellFill::Universe(u)
                    }
                    (None, None) => panic!("cell {id}: fill {fid} is neither lattice nor universe"),
                }
            }
            (Some(_), Some(_)) => panic!("cell {id} has both a material and a fill"),
            (None, None) => panic!("cell {id} has neither a material nor a fill"),
        };
        let cell = match fill {
            CellFill::Material(m) => Cell::material(id, region, m, TEMP_K),
            other => Cell::fill(id, region, other, Position::ZERO),
        };
        universes[uni_index[&uid]].cell_indices.push(cells.len());
        cells.push(cell);
    }

    let model = Model {
        lattices: model_lattices,
        wrapper,
        uni_fill,
        uni_ids: uni_ids.clone(),
    };

    let root = uni_index[&0];
    (
        Geometry {
            surfaces,
            cells,
            universes,
            lattices,
            root_universe: root,
        },
        model,
    )
}

/// The one-line summary the examples print after the canonical build.
pub fn geometry_summary(geom: &Geometry) -> String {
    format!(
        "\n  geometry: {} surfaces, {} cells, {} universes, {} lattices (root = universe 0)",
        geom.surfaces.len(),
        geom.cells.len(),
        geom.universes.len(),
        geom.lattices.len()
    )
}

/// What [`check_geometry`] measured.
#[derive(Debug, Clone)]
pub struct GeometryCheck {
    /// Points at which the two row orders were compared as y-mirrors.
    pub mirror_points: usize,
    /// Points at which the CSG was compared with the hand arithmetic.
    pub points: usize,
    /// Of those, inside the core.
    pub inside: usize,
    /// Of those, outside the vacuum boundary.
    pub outside: usize,
    /// Volume share of each material among the inside points.
    pub shares: Vec<f64>,
}

impl GeometryCheck {
    /// The two lines the examples print: the row-convention result and the
    /// point check with volume shares.
    pub fn report(&self, materials: &[Material]) -> [String; 2] {
        [
            format!(
                "  lattice row convention: the two orders are exact y-mirrors of each other \
                 ({} points), so k cannot depend on the choice",
                self.mirror_points
            ),
            format!(
                "  geometry check: {} points, {} inside / {} outside; volume shares {}",
                self.points,
                self.inside,
                self.outside,
                materials
                    .iter()
                    .zip(&self.shares)
                    .map(|(m, v)| format!("{} {:.1} %", m.name, 100.0 * v))
                    .collect::<Vec<_>>()
                    .join(" / ")
            ),
        ]
    }
}

/// Whitespace-separated numbers inside `<tag> … </tag>`.
pub fn block<T: FromStr>(body: &str, tag: &str) -> Vec<T> {
    elements(body, tag)
        .first()
        .unwrap_or_else(|| panic!("missing <{tag}>"))
        .1
        .split_whitespace()
        .map(|t| num(t, tag))
        .collect()
}

/// A region in these files is a space-separated list of signed surface ids, all
/// intersected — no unions, complements or parentheses appear. Anything else is
/// rejected loudly rather than silently mis-parsed.
pub fn parse_region(
    text: &str,
    surf_index: &BTreeMap<i32, usize>,
    cell_id: i32,
) -> Vec<RegionToken> {
    let mut tokens = Vec::new();
    for t in text.split_whitespace() {
        assert!(
            t.chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == '+'),
            "cell {cell_id}: region token {t:?} is not a signed surface id \
             (unions/complements are not handled)"
        );
        let signed: i32 = num(t, "region surface");
        let idx = *surf_index
            .get(&signed.abs())
            .unwrap_or_else(|| panic!("cell {cell_id}: region names unknown surface {signed}"));
        let sense = if signed < 0 {
            HalfSpaceSense::Inside
        } else {
            HalfSpaceSense::Outside
        };
        let first = tokens.is_empty();
        tokens.push(RegionToken::HalfSpace {
            surface_idx: idx,
            sense,
        });
        if !first {
            tokens.push(RegionToken::Intersection);
        }
    }
    assert!(!tokens.is_empty(), "cell {cell_id}: empty region");
    tokens
}

// ---------------------------------------------------------------------------
// Self-check
// ---------------------------------------------------------------------------

/// Check the CSG descent against hand-written arithmetic at 200 000 points, and
/// settle the lattice row convention.
///
/// This is the first nested-lattice model in this crate, and two of its steps
/// are exactly the kind that produce a plausible wrong answer rather than a
/// crash: the two-level tile index arithmetic, and OpenMC's top-down
/// `<universes>` row order against this crate's bottom-up flat index. The pin
/// *maps* are data read from the same file either way, so what is verified here
/// is the machinery around them.
///
/// 1. **The row convention cannot matter for this model, and that is checked,
///    not assumed.** Every lattice here is centred on its own frame
///    (`lower_left = −½·n·pitch`, asserted while parsing) and every lattice
///    frame is itself centred — the core lattice on the origin, each assembly
///    on its core tile's centre. Reversing a centred lattice's rows therefore
///    mirrors its contents about its own mid-plane, and composing that at both
///    levels is exactly a **global mirror of the model about `y = 0`**. The only
///    other geometry is a z-cylinder on the axis and two z-planes, all
///    y-symmetric, so the two conventions give congruent models and identical
///    `k`. The check builds the model both ways and requires
///    `locate(x, y, z)` under one to equal `locate(x, −y, z)` under the other at
///    every sampled point — which both proves the mirror claim and, because the
///    two builds differ in every lattice, exercises the index arithmetic
///    against an independent transformation.
///
///    (An earlier version of this check tried to pick the convention by
///    measuring which one gave a smoother circular core boundary. It returned
///    exactly the same raggedness for both — 1.944 cm — which is not a weak
///    signal but the mirror symmetry above showing up: no metric invariant
///    under reflection can distinguish them. The convention used is still
///    OpenMC's documented top-down row order; it simply has no effect here.)
///
/// 2. **Point agreement.** For each sampled point, an independent predicate
///    computes the core tile, the assembly tile and the pin region by its own
///    arithmetic — with the pin radii read by hand off the model's surface
///    cards — and the resulting material must equal the one
///    `Geometry::locate` descends to, including `None` outside the vacuum
///    boundary.
pub fn check_geometry(case: Case, geom: &Geometry, model: &Model, materials: &[Material]) -> GeometryCheck {
    let core = model.lattices.get(&99).expect("core lattice 99");

    // --- 1. the row convention is a global y-mirror -----------------------
    {
        let mirrored = build_geometry_and_model(case, materials, false).0;
        let mut seed = 777_777_u64;
        let mut prn = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };
        const M: usize = 50_000;
        let u = Direction::new(0.0, 0.0, 1.0);
        for _ in 0..M {
            let (x, y, z) = (
                (2.0 * prn() - 1.0) * R_CORE * 1.02,
                (2.0 * prn() - 1.0) * R_CORE * 1.02,
                Z_LO - 1.0 + prn() * (Z_HI - Z_LO + 2.0),
            );
            let a = geom
                .locate(Position::new(x, y, z), u, SurfaceToken::NONE)
                .and_then(|p| p.material);
            let b = mirrored
                .locate(Position::new(x, -y, z), u, SurfaceToken::NONE)
                .and_then(|p| p.material);
            assert_eq!(
                a, b,
                "the two row conventions are not mirror images at ({x}, {y}, {z}): {a:?} vs {b:?}"
            );
        }
    }
    const M: usize = 50_000;

    // --- 1. point agreement ----------------------------------------------
    // Materials are listed as ids 1..n in order (asserted in `main`), so a
    // model id maps to a slot by subtracting one. `PIN_SHELLS` is the hand-read
    // transcription of each pin universe.
    let slot = |material_id: i32| -> usize { (material_id - 1) as usize };
    let shells = |uid: i32| -> &'static (i32, &'static [(f64, i32)], i32) {
        PIN_SHELLS
            .iter()
            .find(|(u, _, _)| *u == uid)
            .unwrap_or_else(|| panic!("pin universe {uid} is not in PIN_SHELLS"))
    };
    let expect = |p: Position| -> Option<usize> {
        if p.z <= Z_LO || p.z >= Z_HI || p.x * p.x + p.y * p.y >= R_CORE * R_CORE {
            return None;
        }
        // core tile
        let cx = ((p.x - core.lower_left.x) / core.pitch[0]).floor();
        let cy = ((p.y - core.lower_left.y) / core.pitch[1]).floor();
        assert!(
            cx >= 0.0 && cy >= 0.0 && cx < core.n[0] as f64 && cy < core.n[1] as f64,
            "core cylinder reaches outside the 7×7 lattice at {p:?}"
        );
        let (cx, cy) = (cx as usize, cy as usize);
        let uid = model.uni_ids[core.universes[core.n[0] * cy + cx]];
        // A core tile is either an assembly (its wrapper universe is filled by
        // a lattice) or the all-water tile (filled by pin universe 1).
        let lid = match (model.wrapper.get(&uid), model.uni_fill.get(&uid)) {
            (Some(&l), _) => l,
            (None, Some(&1)) => return Some(slot(1)), // the all-water tile
            _ => panic!("core tile {cx},{cy} is universe {uid}, which fills nothing known"),
        };
        let asm = &model.lattices[&lid];
        let x0 = core.lower_left.x + (cx as f64 + 0.5) * core.pitch[0];
        let y0 = core.lower_left.y + (cy as f64 + 0.5) * core.pitch[1];
        let (lx, ly) = (p.x - x0, p.y - y0);
        let ax = ((lx - asm.lower_left.x) / asm.pitch[0]).floor();
        let ay = ((ly - asm.lower_left.y) / asm.pitch[1]).floor();
        assert!(
            ax >= 0.0 && ay >= 0.0 && ax < asm.n[0] as f64 && ay < asm.n[1] as f64,
            "assembly {lid} does not tile its core cell at {p:?}"
        );
        let (ax, ay) = (ax as usize, ay as usize);
        let pin_uid = model.uni_ids[asm.universes[asm.n[0] * ay + ax]];
        let (_, rings, outside_mat) = shells(pin_uid);
        // pin universe 2: fuel / clad / water by radius about the pin centre
        let px = asm.lower_left.x + (ax as f64 + 0.5) * asm.pitch[0];
        let py = asm.lower_left.y + (ay as f64 + 0.5) * asm.pitch[1];
        let (dx, dy) = (lx - px, ly - py);
        let r = (dx * dx + dy * dy).sqrt();
        for &(r_out, mat_id) in rings.iter() {
            if r < r_out {
                return Some(slot(mat_id));
            }
        }
        Some(slot(*outside_mat))
    };

    let mut seed = 20_260_911_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    const N: usize = 200_000;
    let (mut inside, mut outside) = (0usize, 0usize);
    let mut tally = vec![0usize; materials.len()];
    for _ in 0..N {
        let p = Position::new(
            (2.0 * prn() - 1.0) * R_CORE * 1.05,
            (2.0 * prn() - 1.0) * R_CORE * 1.05,
            Z_LO - 2.0 + prn() * (Z_HI - Z_LO + 4.0),
        );
        let want = expect(p);
        let got = geom
            .locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .and_then(|path| path.material);
        assert_eq!(
            got, want,
            "CSG disagrees with the model at {p:?}: found material {got:?}, model says {want:?}"
        );
        match want {
            Some(m) => {
                inside += 1;
                tally[m] += 1;
            }
            None => outside += 1,
        }
    }
    let shares: Vec<f64> = tally.iter().map(|&t| t as f64 / inside as f64).collect();
    assert!(
        inside > N / 10 && outside > N / 100 && tally.iter().all(|&t| t > 0),
        "degenerate sampling"
    );
    GeometryCheck { mirror_points: M, points: N, inside, outside, shares }
}

/// Fuel pellet radius [cm] — surface 1 in the model.
pub const R_FUEL: f64 = 0.514858;
/// Clad outer radius [cm] — surface 2 in the model.
pub const R_CLAD: f64 = 0.602996;

// ---------------------------------------------------------------------------
// The pin in a reflective square cell (k_inf)
// ---------------------------------------------------------------------------
//
// From `examples/common/lct008_pin_cell.rs` (moved there from
// `lct008_pitch_sweep.rs` on 2026-10-05, here on 2026-11-17), unchanged.
// Shared by `lct008_pitch_sweep.rs` (the recorded sweep,
// `verification_and_validation/tutorial_rung4/pitch_sweep.md`) and the
// `dhoby-ghaut` Monte Carlo web demo's pitch slider (gh:#549). Material slots
// are the case-1 order: water 0, fuel 1, clad 2. No gap: LCT-008's pin has
// none.

/// Half-height of the reflective cell [cm]. With reflective z-planes the value
/// does not change k∞; it only bounds the initial source box.
pub const HALF_Z: f64 = 10.0;

/// The moderator-to-fuel volume ratio of a square pitch `pitch` [cm].
pub fn vm_over_vf(pitch: f64, r_fuel: f64, r_clad: f64) -> f64 {
    (pitch * pitch - core::f64::consts::PI * r_clad * r_clad) / (core::f64::consts::PI * r_fuel * r_fuel)
}

/// The initial-source box: the fuel's bounding box over the cell's height.
pub fn pin_source_box(r_fuel: f64) -> SourceBox {
    SourceBox { lower: Position::new(-r_fuel, -r_fuel, -HALF_Z), upper: Position::new(r_fuel, r_fuel, HALF_Z) }
}

/// The reflective square pin cell of half-pitch `half` [cm], centred on the
/// origin: fuel to `r_fuel`, clad to `r_clad`, water outside, every cell at
/// `temp_k`.
pub fn pin_cell(half: f64, r_fuel: f64, r_clad: f64, temp_k: f64) -> Geometry {
    let hs = |surface_idx: usize, inside: bool| RegionToken::HalfSpace {
        surface_idx,
        sense: if inside { HalfSpaceSense::Inside } else { HalfSpaceSense::Outside },
    };
    let cyl = |r: f64| SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r, bc: BoundaryType::Transmissive });
    let refl = BoundaryType::Reflective;
    let surfaces = vec![
        cyl(r_fuel),                                           // 0
        cyl(r_clad),                                           // 1
        SurfaceKind::XPlane(XPlane { x0: -half, bc: refl }),   // 2
        SurfaceKind::XPlane(XPlane { x0: half, bc: refl }),    // 3
        SurfaceKind::YPlane(YPlane { y0: -half, bc: refl }),   // 4
        SurfaceKind::YPlane(YPlane { y0: half, bc: refl }),    // 5
        SurfaceKind::ZPlane(ZPlane { z0: -HALF_Z, bc: refl }), // 6
        SurfaceKind::ZPlane(ZPlane { z0: HALF_Z, bc: refl }),  // 7
    ];
    // Every cell is bounded by the box, so nothing is defined outside it.
    let in_box = |mut r: Vec<RegionToken>| {
        for (s, inside) in [(2, false), (3, true), (4, false), (5, true), (6, false), (7, true)] {
            r.push(hs(s, inside));
            r.push(RegionToken::Intersection);
        }
        r
    };
    let cells = vec![
        Cell::material(1, in_box(vec![hs(0, true)]), 1, temp_k),
        Cell::material(2, in_box(vec![hs(0, false), hs(1, true), RegionToken::Intersection]), 2, temp_k),
        Cell::material(3, in_box(vec![hs(1, false)]), 0, temp_k),
    ];
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1, 2] }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case1_materials() -> Vec<Material> {
        let spec = parse_materials(Case::One.materials_xml());
        // Every card nuclide in its own slot: no omissions, no data needed.
        let mut slots = BTreeMap::new();
        for m in &spec {
            for (n, _) in &m.nuclides {
                let k = slots.len();
                slots.entry(n.clone()).or_insert(k);
            }
        }
        build_materials(&spec, &slots, &BTreeMap::new(), false).0
    }

    /// The cards parse into case 1's three materials, and the full
    /// self-check (y-mirror at 50 000 points, hand arithmetic at 200 000)
    /// passes on the canonical build.
    #[test]
    fn case1_builds_and_passes_its_self_check() {
        let mats = case1_materials();
        assert_eq!(mats.len(), 3);
        let (geom, model) = build_geometry_and_model(Case::One, &mats, true);
        let check = check_geometry(Case::One, &geom, &model, &mats);
        assert_eq!((check.points, check.mirror_points), (200_000, 50_000));
        assert!((check.shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(geometry_summary(&geom).contains("lattices"));
        assert_eq!(check.report(&mats).len(), 2);
    }

    /// Case 8's cards hold the pyrex material (universe 3) and parse.
    #[test]
    fn case8_has_a_fourth_material() {
        assert_eq!(Case::from_number(8), Case::Eight);
        assert_eq!(Case::from_number(5), Case::One);
        assert_eq!(parse_materials(Case::Eight.materials_xml()).len(), 4);
        assert_eq!(parse_materials(Case::Two.materials_xml()).len(), 3);
    }

    /// The omission report names what is left out, or says nothing is.
    #[test]
    fn omissions_are_reported() {
        let spec = parse_materials(Case::One.materials_xml());
        assert_eq!(omissions_report(&spec, &BTreeMap::new()).len(), 1);
        let mut omitted = BTreeMap::new();
        omitted.insert("Zn64".to_string(), 1.0e-6);
        let lines = omissions_report(&spec, &omitted);
        assert!(lines.last().unwrap().contains("Zn64"));
    }

    /// The pin cell's ratio at LCT-008's pitch, and its geometry's shape.
    #[test]
    fn pin_cell_at_the_lattice_pitch() {
        let v = vm_over_vf(1.63576, R_FUEL, R_CLAD);
        assert!((v - (1.63576f64 * 1.63576 - core::f64::consts::PI * R_CLAD * R_CLAD) / (core::f64::consts::PI * R_FUEL * R_FUEL)).abs() < 1e-15);
        let g = pin_cell(0.5 * 1.63576, R_FUEL, R_CLAD, TEMP_K);
        assert_eq!((g.surfaces.len(), g.cells.len()), (8, 3));
        assert_eq!(pin_source_box(R_FUEL).upper.z, HALF_Z);
    }
}
