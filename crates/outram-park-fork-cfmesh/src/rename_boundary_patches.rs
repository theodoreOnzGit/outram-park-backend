// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/surfaceTools/renameBoundaryPatches/
//     renameBoundaryPatches.{C,H}
//   meshLibrary/utilities/meshes/polyMeshGenModifier/
//     polyMeshGenModifierReplaceBoundary.C (serial part)
//   meshLibrary/utilities/meshes/polyMeshGen/polyMeshGenFaces.C (findPatches)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! cfMesh's **`renameBoundaryPatches`** (gh:#1272): `meshDict`'s
//! `renameBoundary` applied to the mesh's patches, the generators' last step.
//!
//! - Each `newPatchNames` entry matches patch names by regular expression
//!   (`findPatches`, a full match); every match gets the entry's `newName`
//!   (default: its own name) and `type` (default `wall`). Entries that give
//!   the same new name merge into one patch.
//! - With `defaultName`, every patch no entry matched goes into one patch of
//!   that name and `defaultType` (default `wall`); without it, such patches
//!   keep their name and type.
//! - Boundary faces are then regrouped by new patch (`replaceBoundary`),
//!   keeping their order within each patch.
//! - `checkEmptyPatches`: type `empty` becomes `wall` unless empty patches
//!   are allowed (the generators do not allow them).
//!
//! [`VolumeMesh`] carries no patch type, so the types travel beside the mesh
//! as a list, one per patch.
//!
//! ~~**Not ported:** `checkSymmetryPlanes` (`symmetryPlaneOptimisation`, which
//! flattens patches of type `symmetryPlane`); [`rename_boundary_patches`]
//! reports the `symmetryPlane` patches it did not flatten.~~ **UPDATED
//! 2026-11-05 (gh:#1280):** `checkSymmetryPlanes` runs
//! [`crate::symmetry_plane_optimisation`] last, as upstream; its report is
//! [`RenamedBoundary::symmetry`]. The regular
//! expressions support literals, `.`, bracket classes, `*`, `+`, `?`, `|`
//! and groups `( )`, the subset `meshDict` patterns use; anything else is
//! taken literally.
//!
//! **Upstream quirks kept.** An entry that is not a dictionary stops the
//! renaming at that entry, with the patches already renamed left as they
//! were (upstream warns and returns before `replaceBoundary`). Patches kept
//! under their own name (no `defaultName`) are not checked against new
//! names, so a kept name can repeat a new one.

use crate::symmetry_plane_optimisation::{symmetry_plane_optimisation, SymmetryReport};
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};

/// One `newPatchNames` entry's dictionary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewPatch {
    /// `newName`.
    pub new_name: Option<String>,
    /// `type`.
    pub patch_type: Option<String>,
}

/// `meshDict`'s `renameBoundary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenameBoundary {
    /// `newPatchNames`: (pattern, dictionary); `None` is an entry that is not
    /// a dictionary.
    pub new_patch_names: Vec<(String, Option<NewPatch>)>,
    /// `defaultName`.
    pub default_name: Option<String>,
    /// `defaultType`.
    pub default_type: Option<String>,
}

/// The renamed mesh, its patch types and what was not done.
#[derive(Debug, Clone)]
pub struct RenamedBoundary {
    /// The mesh with the new patches.
    pub mesh: VolumeMesh,
    /// The type of each patch.
    pub patch_types: Vec<String>,
    /// Patterns that matched no patch (upstream warns).
    pub unmatched: Vec<String>,
    /// `checkSymmetryPlanes` (`symmetryPlaneOptimisation`, gh:#1280).
    pub symmetry: SymmetryReport,
}

/// `renameBoundaryPatches(mesh, meshDict, allowEmptyPatches)`.
pub fn rename_boundary_patches(mesh: &VolumeMesh, patch_types: &[String], dict: Option<&RenameBoundary>, allow_empty_patches: bool) -> RenamedBoundary {
    let mut out = mesh.clone();
    let mut types: Vec<String> = patch_types.to_vec();
    types.resize(mesh.patches.len(), "patch".to_string());
    let mut unmatched = Vec::new();
    if let Some(d) = dict {
        if let Some((m, t, u)) = calculate_new_boundary(mesh, &types, d) {
            out = m;
            types = t;
            unmatched = u;
        }
    }
    if !allow_empty_patches {
        for t in types.iter_mut() {
            if t == "empty" {
                *t = "wall".into();
            }
        }
    }
    let symmetry = symmetry_plane_optimisation(&mut out, &types);
    RenamedBoundary { mesh: out, patch_types: types, unmatched, symmetry }
}

/// `calculateNewBoundary`; `None` when it returns without replacing.
fn calculate_new_boundary(mesh: &VolumeMesh, types: &[String], dict: &RenameBoundary) -> Option<(VolumeMesh, Vec<String>, Vec<String>)> {
    let n = mesh.patches.len();
    let mut patch_to_new: Vec<Option<usize>> = vec![None; n];
    let mut new_names: Vec<String> = Vec::new();
    let mut new_types: Vec<String> = Vec::new();
    let mut unmatched = Vec::new();
    for (pattern, entry) in &dict.new_patch_names {
        let matched: Vec<usize> = (0..n).filter(|&p| regex_full_match(pattern, &mesh.patches[p].name)).collect();
        if matched.is_empty() {
            unmatched.push(pattern.clone());
            continue;
        }
        let Some(pd) = entry else {
            // "Cannot rename patch ... Exitting."
            return None;
        };
        for &p in &matched {
            let new_name = pd.new_name.clone().unwrap_or_else(|| mesh.patches[p].name.clone());
            if let Some(pos) = new_names.iter().position(|x| *x == new_name) {
                patch_to_new[p] = Some(pos);
                continue;
            }
            new_names.push(new_name);
            new_types.push(pd.patch_type.clone().unwrap_or_else(|| "wall".into()));
            patch_to_new[p] = Some(new_names.len() - 1);
        }
    }
    let default_name = dict.default_name.clone().unwrap_or_else(|| "walls".into());
    let default_type = dict.default_type.clone().unwrap_or_else(|| "wall".into());
    let mut default_patch = None;
    if dict.default_name.is_some() && new_names.len() < n {
        if patch_to_new.iter().any(|x| x.is_none()) {
            new_names.push(default_name);
            new_types.push(default_type);
            default_patch = Some(new_names.len() - 1);
        }
    } else {
        for p in 0..n {
            if patch_to_new[p].is_none() {
                patch_to_new[p] = Some(new_names.len());
                new_names.push(mesh.patches[p].name.clone());
                new_types.push(types[p].clone());
            }
        }
    }
    if new_names.is_empty() {
        return None;
    }
    // replaceBoundary: faces grouped by new patch, order kept within a patch
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); new_names.len()];
    for (p, pa) in mesh.patches.iter().enumerate() {
        // a patch no entry matched goes to the last (default) patch
        let target = patch_to_new[p].or(default_patch).unwrap_or(new_names.len() - 1);
        groups[target].extend(pa.start_face..pa.start_face + pa.n_faces);
    }
    let ni = mesh.n_internal_faces();
    let mut faces: Vec<Vec<usize>> = mesh.faces[..ni].to_vec();
    let mut owner: Vec<usize> = mesh.owner[..ni].to_vec();
    let mut neighbour: Vec<Option<usize>> = mesh.neighbour[..ni].to_vec();
    let mut patches = Vec::new();
    for (g, fs) in groups.iter().enumerate() {
        patches.push(BoundaryPatch { name: new_names[g].clone(), start_face: faces.len(), n_faces: fs.len() });
        for &f in fs {
            faces.push(mesh.faces[f].clone());
            owner.push(mesh.owner[f]);
            neighbour.push(None);
        }
    }
    let m = VolumeMesh { points: mesh.points.clone(), faces, owner, neighbour, n_cells: mesh.n_cells, patches , cells: None };
    Some((m, new_types, unmatched))
}

/// One regular-expression atom.
#[derive(Debug, Clone, PartialEq)]
enum Atom {
    Lit(char),
    Any,
    Class(Vec<(char, char)>, bool),
    Group(Vec<Vec<(Atom, Quant)>>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Quant {
    One,
    Star,
    Plus,
    Opt,
}

fn parse_alts(s: &[char], i: &mut usize) -> Vec<Vec<(Atom, Quant)>> {
    let mut alts = vec![Vec::new()];
    while *i < s.len() {
        let c = s[*i];
        let atom = match c {
            ')' => break,
            '|' => {
                *i += 1;
                alts.push(Vec::new());
                continue;
            }
            '(' => {
                *i += 1;
                let g = parse_alts(s, i);
                if *i < s.len() && s[*i] == ')' {
                    *i += 1;
                }
                Atom::Group(g)
            }
            '.' => {
                *i += 1;
                Atom::Any
            }
            '[' => {
                *i += 1;
                let neg = *i < s.len() && s[*i] == '^';
                if neg {
                    *i += 1;
                }
                let mut ranges = Vec::new();
                while *i < s.len() && s[*i] != ']' {
                    let a = s[*i];
                    if *i + 2 < s.len() && s[*i + 1] == '-' && s[*i + 2] != ']' {
                        ranges.push((a, s[*i + 2]));
                        *i += 3;
                    } else {
                        ranges.push((a, a));
                        *i += 1;
                    }
                }
                *i += 1;
                Atom::Class(ranges, neg)
            }
            '\\' if *i + 1 < s.len() => {
                *i += 2;
                Atom::Lit(s[*i - 1])
            }
            _ => {
                *i += 1;
                Atom::Lit(c)
            }
        };
        let q = match s.get(*i) {
            Some('*') => Quant::Star,
            Some('+') => Quant::Plus,
            Some('?') => Quant::Opt,
            _ => Quant::One,
        };
        if q != Quant::One {
            *i += 1;
        }
        alts.last_mut().expect("one alternative").push((atom, q));
    }
    alts
}

/// The end positions after matching `seq` from `pos` in `t`.
fn match_seq(seq: &[(Atom, Quant)], t: &[char], pos: usize) -> Vec<usize> {
    let Some(((atom, q), rest)) = seq.split_first() else {
        return vec![pos];
    };
    let once = |p: usize| -> Vec<usize> {
        match atom {
            Atom::Lit(c) => (p < t.len() && t[p] == *c).then_some(p + 1).into_iter().collect(),
            Atom::Any => (p < t.len()).then_some(p + 1).into_iter().collect(),
            Atom::Class(r, neg) => (p < t.len() && r.iter().any(|&(a, b)| a <= t[p] && t[p] <= b) != *neg).then_some(p + 1).into_iter().collect(),
            Atom::Group(alts) => alts.iter().flat_map(|a| match_seq(a, t, p)).collect(),
        }
    };
    let mut starts: Vec<usize> = Vec::new();
    match q {
        Quant::One => starts = once(pos),
        Quant::Opt => {
            starts.push(pos);
            starts.extend(once(pos));
        }
        Quant::Star | Quant::Plus => {
            let mut frontier = vec![pos];
            if *q == Quant::Star {
                starts.push(pos);
            }
            while !frontier.is_empty() {
                let mut nxt = Vec::new();
                for p in frontier {
                    for e in once(p) {
                        if !starts.contains(&e) && e != p {
                            starts.push(e);
                            nxt.push(e);
                        }
                    }
                }
                frontier = nxt;
            }
        }
    }
    let mut out: Vec<usize> = starts.into_iter().flat_map(|p| match_seq(rest, t, p)).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Full match of `name` by the regular expression `pattern` (module docs).
pub fn regex_full_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = name.chars().collect();
    let mut i = 0;
    let alts = parse_alts(&p, &mut i);
    alts.iter().any(|a| match_seq(a, &t, 0).contains(&t.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::math::Vec3;
    use crate::patches::{assign_patches_by_region, SurfaceRegions};
    use crate::shapes::box_surface;

    #[test]
    fn regex_subset() {
        assert!(regex_full_match("wall.*", "walls"));
        assert!(regex_full_match("inlet|outlet", "outlet"));
        assert!(!regex_full_match("inlet", "inlet2"));
        assert!(regex_full_match("p[0-9]+", "p12"));
        assert!(!regex_full_match("p[0-9]+", "p"));
        assert!(regex_full_match("(in|out)let_?a", "inleta"));
        assert!(regex_full_match("a\\.b", "a.b") && !regex_full_match("a\\.b", "axb"));
    }

    /// V&V (gh:#1272), analytic against `renameBoundaryPatches.C`: a carved
    /// box with patches `walls`, `outlet`, `inlet` (16, 4, 4 faces). Renaming
    /// `in.*|out.*` to `ends` (type `patch`) merges inlet and outlet; with
    /// `defaultName` `sides` the rest goes there; without it `walls` keeps
    /// its name and type; an empty type becomes wall; an entry that is not a
    /// dictionary leaves the mesh as it was.
    #[test]
    fn rename_boundary_patches_follows_upstream() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&["walls", "walls", "walls", "walls", "walls", "walls", "walls", "walls", "outlet", "outlet", "inlet", "inlet"]);
        let m = assign_patches_by_region(&carve_box(&p, &t, 0.5), &p, &t, &regions).unwrap();
        let types = vec!["patch".to_string(), "patch".into(), "empty".into()];
        let ends = RenameBoundary {
            new_patch_names: vec![("in.*|out.*".into(), Some(NewPatch { new_name: Some("ends".into()), patch_type: Some("patch".into()) }))],
            default_name: Some("sides".into()),
            default_type: None,
        };
        let r = rename_boundary_patches(&m, &types, Some(&ends), false);
        r.mesh.validate().unwrap();
        let names: Vec<(&str, usize)> = r.mesh.patches.iter().map(|p| (p.name.as_str(), p.n_faces)).collect();
        assert_eq!(names, vec![("ends", 8), ("sides", 16)]);
        assert_eq!(r.patch_types, vec!["patch".to_string(), "wall".into()]);
        assert!((r.mesh.total_volume() - 1.0).abs() < 1e-12);
        let keep = RenameBoundary { default_name: None, ..ends.clone() };
        let r = rename_boundary_patches(&m, &types, Some(&keep), false);
        let names: Vec<(&str, usize)> = r.mesh.patches.iter().map(|p| (p.name.as_str(), p.n_faces)).collect();
        assert_eq!(names, vec![("ends", 8), ("walls", 16)]);
        assert_eq!(r.patch_types, vec!["patch".to_string(), "patch".into()]);
        // no renameBoundary: names kept, empty -> wall
        let r = rename_boundary_patches(&m, &types, None, false);
        assert!(crate::tet_mesh_extractor_octree::same_mesh(&r.mesh, &m));
        assert_eq!(r.patch_types, vec!["patch".to_string(), "patch".into(), "wall".into()]);
        // an entry that is not a dictionary: nothing renamed
        let bad = RenameBoundary { new_patch_names: vec![("walls".into(), None)], ..Default::default() };
        let r = rename_boundary_patches(&m, &types, Some(&bad), true);
        assert!(crate::tet_mesh_extractor_octree::same_mesh(&r.mesh, &m));
        assert_eq!(r.patch_types[2], "empty");
        let none = RenameBoundary { new_patch_names: vec![("nothing".into(), Some(NewPatch::default()))], ..Default::default() };
        assert_eq!(rename_boundary_patches(&m, &types, Some(&none), false).unmatched, vec!["nothing".to_string()]);
    }
}
