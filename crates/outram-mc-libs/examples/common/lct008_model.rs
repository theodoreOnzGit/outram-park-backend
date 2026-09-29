// SPDX-License-Identifier: GPL-3.0

//! **The LEU-COMP-THERM-008 lattice model, shared by the examples that run it.**
//!
//! Moved verbatim out of `lct008_keff.rs` on 2026-09-29 so that `lct008_keff.rs`,
//! `lct008_ace_roundtrip.rs` and `icsbep_five_route_keff.rs` run ONE model: the
//! committed `mit-crpg/benchmarks` OpenMC cards for cases 1, 2 and 8, parsed at
//! run time into materials and a CSG lattice, plus the geometry self-check.
//! Each example pulls it in with `#[path = "common/lct008_model.rs"] mod
//! lct008_model;`. A subdirectory without `main.rs` is not an example target,
//! so Cargo does not build this file on its own.
//!
//! The provenance, the approximations and every recorded result stay in
//! `lct008_keff.rs`'s module documentation, which is the authority for the model.

#![allow(dead_code)]

#[allow(unused_imports)]
use njoy_outram_park_fork::reference_data::reference_endf;
#[allow(unused_imports)]
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
#[allow(unused_imports)]
use outram_mc_libs::geometry::geometry::Geometry;
#[allow(unused_imports)]
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
#[allow(unused_imports)]
use outram_mc_libs::geometry::position::{Direction, Position};
#[allow(unused_imports)]
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCylinder, ZPlane};
#[allow(unused_imports)]
use outram_mc_libs::geometry::universe::Universe;
#[allow(unused_imports)]
use outram_mc_libs::material::material::{Material, NuclideComponent};
#[allow(unused_imports)]
use outram_mc_libs::material::nuclide::Nuclide;
#[allow(unused_imports)]
use outram_mc_libs::material::thermal::ThermalScattering;
#[allow(unused_imports)]
use outram_mc_libs::physics::compute::ComputeType;
#[allow(unused_imports)]
use outram_mc_libs::physics::keff::KeffSettings;
#[allow(unused_imports)]
use outram_mc_libs::physics::reactor_physics::{run_keff_reactor_physics, ReactorPhysicsConfig};
#[allow(unused_imports)]
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::time::Instant;

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

/// The committed case, selected by `--case`.
///
/// Every case of LEU-COMP-THERM-008 is **independently critical**, and they
/// differ in how the poison is supplied: case 1 carries 1511 ppm of *soluble*
/// boron and no poison rods; case 8 carries 794 ppm and 144 *lumped* pyrex
/// burnable-poison rods. Same lattice, same pitch, same fuel. That makes the
/// pair a **differential measurement needing no new reference**: an error in
/// thermal absorption tracks the soluble-boron worth and its distribution, while
/// an error in epithermal resonance escape does not, because `p` is set by the
/// lattice and the lattice is unchanged.
pub static ACTIVE_CASE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

pub fn materials_xml() -> &'static str {
    match ACTIVE_CASE.get().copied().unwrap_or(1) {
        2 => MATERIALS_XML_CASE2,
        8 => MATERIALS_XML_CASE8,
        _ => MATERIALS_XML_CASE1,
    }
}
pub fn geometry_xml() -> &'static str {
    match ACTIVE_CASE.get().copied().unwrap_or(1) {
        2 => GEOMETRY_XML_CASE2,
        8 => GEOMETRY_XML_CASE8,
        _ => GEOMETRY_XML_CASE1,
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
/// It exercises the identical transport path at a fraction of the
/// reconstruction cost, so it is the tier to use while iterating on geometry,
/// tallies or statistics. It is **not** the default: the model it describes is
/// incomplete, and correct physics is the default here.
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

pub fn num<T: std::str::FromStr>(s: &str, what: &str) -> T {
    s.trim()
        .parse()
        .unwrap_or_else(|_| panic!("{what}: cannot parse {s:?}"))
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

pub struct MaterialSpec {
    pub id: i32,
    pub name: String,
    pub nuclides: Vec<(String, f64)>,
    pub sab: Option<String>,
}

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

pub fn report_omissions(spec: &[MaterialSpec], omitted: &BTreeMap<String, f64>) {
    if omitted.is_empty() {
        eprintln!("  every nuclide in the model has a tape — nothing omitted.");
        return;
    }
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
            eprintln!(
                "  material {} \"{}\": omitted {:.3e} of {:.3e} /b·cm ({:.2} % of its atoms)",
                m.id,
                m.name,
                miss,
                sum,
                100.0 * miss / sum
            );
        }
    }
    eprintln!(
        "  omitted (not in the active nuclide tier, or no tape): {} — {:.3e} /b·cm in total",
        names.join(", "),
        total
    );
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Core bounding cylinder [cm] — surface 30 in the model, vacuum.
pub const R_CORE: f64 = 76.200;
/// Core axial extent [cm] — surfaces 10 and 20, both vacuum.
pub const Z_LO: f64 = -81.662;
pub const Z_HI: f64 = 81.662;

/// The parsed model, kept so the self-check can re-derive lattice lookups by
/// its own arithmetic instead of trusting `Geometry::locate`.
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

// The parse is done once and stashed, because `build_geometry` and
// `check_geometry` both need it and the model is immutable.
pub static MODEL: std::sync::OnceLock<Model> = std::sync::OnceLock::new();

pub fn build_geometry(materials: &[Material], reverse_rows: bool) -> Geometry {
    let xml = strip_comments(geometry_xml());

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

    let _ = MODEL.set(Model {
        lattices: model_lattices,
        wrapper,
        uni_fill,
        uni_ids: uni_ids.clone(),
    });

    let root = uni_index[&0];
    // Only the canonical build reports; `check_geometry` builds the mirrored
    // variant a second time and there is nothing new to say about it.
    if reverse_rows {
        eprintln!(
            "\n  geometry: {} surfaces, {} cells, {} universes, {} lattices (root = universe 0)",
            surfaces.len(),
            cells.len(),
            universes.len(),
            lattices.len()
        );
    }
    Geometry {
        surfaces,
        cells,
        universes,
        lattices,
        root_universe: root,
    }
}

/// Whitespace-separated numbers inside `<tag> … </tag>`.
pub fn block<T: std::str::FromStr>(body: &str, tag: &str) -> Vec<T> {
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
pub fn check_geometry(geom: &Geometry, materials: &[Material]) -> Vec<f64> {
    let model = MODEL.get().expect("model");
    let core = model.lattices.get(&99).expect("core lattice 99");

    // --- 1. the row convention is a global y-mirror -----------------------
    {
        let mirrored = build_geometry(materials, false);
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
        eprintln!(
            "  lattice row convention: the two orders are exact y-mirrors of each other \
             ({M} points), so k cannot depend on the choice"
        );
    }

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
    eprintln!(
        "  geometry check: {N} points, {inside} inside / {outside} outside; volume shares {}",
        materials
            .iter()
            .zip(&shares)
            .map(|(m, v)| format!("{} {:.1} %", m.name, 100.0 * v))
            .collect::<Vec<_>>()
            .join(" / ")
    );
    assert!(
        inside > N / 10 && outside > N / 100 && tally.iter().all(|&t| t > 0),
        "degenerate sampling"
    );
    shares
}

/// Pin radii [cm] — surfaces 1 and 2 in the model.
pub const R_FUEL: f64 = 0.514858;
pub const R_CLAD: f64 = 0.602996;
