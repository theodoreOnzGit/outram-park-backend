// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (translated to Rust, operation for operation):
//   OpenFOAM ESI v2506 (OpenCFD Ltd.), src/meshTools/topoSet/cellSources/
//   {boxToCell,sphereToCell,cylinderToCell}/*.C and
//   src/meshTools/topoSet/topoSetSource/topoSetSource.C.
//   Copyright (C) 2011-2016 OpenFOAM Foundation, (C) 2017-2023 OpenCFD Ltd.
//   Licence: GPL-3.0-or-later.
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

//! **Cell-selection sources of OpenFOAM's `topoSet`** (gh:#1384): the sources
//! `setFields` and `topoSet` share.
//!
//! A source selects cells by the position of the **cell centre**
//! (`mesh.cellCentres()`, which [`PolyMesh::to_fv_mesh`] reproduces bit for
//! bit with ESI v2506, gh:#1489). The three here are the ones the demos use:
//!
//! | source | selects a cell when | upstream |
//! |---|---|---|
//! | [`CellSource::Box`] | the centre lies in any box, bounds inclusive | `boxToCell` |
//! | [`CellSource::Sphere`] | `innerRadius^2 < |c-o|^2 < radius^2`, bounds exclusive | `sphereToCell` |
//! | [`CellSource::Cylinder`] | the axial coordinate lies in `(0, |axis|^2)` and the radial one in the annulus | `cylinderToCell` |
//!
//! The dictionary keywords follow v2506: `box (min) (max)` / `boxes` /
//! `min`+`max`|`span`; `origin` (alias `centre`), `radius`, `innerRadius`;
//! `point1`|`p1`, `point2`|`p2`, `radius`|`outerRadius`, `innerRadius`.
//! The `coordinateSystem` / `transform` option of `topoSetSource` is **not**
//! ported and is rejected, not ignored.
//!
//! Not ported yet (deferred, gh:#1384): the other cell sources, every face and
//! point source, `topoSet`'s zone conversion.

use std::collections::BTreeSet;

use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamValue};
use outram_foam_basic_lib::primitives::Vector3;

use crate::MeshError;

/// `topoSetSource::setAction` for the actions the selection sources support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetAction {
    /// Start from an empty set, then add the selection.
    New,
    /// Add the selection to the set.
    Add,
    /// Remove the selection from the set.
    Subtract,
}

/// An axis-aligned box, `treeBoundBox`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundBox {
    /// Lower corner.
    pub min: Vector3,
    /// Upper corner.
    pub max: Vector3,
}

impl BoundBox {
    /// `boundBox::contains(point)`: inclusive on all six faces.
    pub fn contains(&self, p: Vector3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }
}

/// A cell-selection source: `boxToCell`, `sphereToCell`, `cylinderToCell`.
#[derive(Debug, Clone, PartialEq)]
pub enum CellSource {
    /// `boxToCell`.
    Box {
        /// The boxes (a cell in any of them is selected).
        boxes: Vec<BoundBox>,
    },
    /// `sphereToCell`.
    Sphere {
        /// Sphere centre.
        origin: Vector3,
        /// Outer radius.
        radius: f64,
        /// Inner radius (`0` = none).
        inner_radius: f64,
    },
    /// `cylinderToCell`.
    Cylinder {
        /// Axis start.
        point1: Vector3,
        /// Axis end.
        point2: Vector3,
        /// Outer radius.
        radius: f64,
        /// Inner radius (`0` = none).
        inner_radius: f64,
    },
}

/// The vectors in an entry: a single `(x y z)` list, or several of them as a
/// bare token sequence (`box (0 0 0) (1 1 1);`).
fn entry_vectors(entry: &FoamEntry) -> Option<Vec<Vector3>> {
    match entry {
        FoamEntry::List(v) => {
            let l = FoamValue::List(v.clone());
            l.as_vector3().map(|p| vec![p])
        }
        FoamEntry::Tokens(vs) => vs.iter().map(FoamValue::as_vector3).collect(),
        _ => None,
    }
}

fn dict_vector(dict: &FoamDict, keys: &[&str]) -> Option<Vector3> {
    keys.iter()
        .find_map(|k| dict.get(k))
        .and_then(entry_vectors)
        .and_then(|v| if v.len() == 1 { Some(v[0]) } else { None })
}

fn dict_scalar(dict: &FoamDict, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| dict.get_scalar(k))
}

fn missing(kind: &str, key: &str) -> MeshError {
    MeshError::DictParse(format!("{kind}: required entry `{key}` is missing or malformed"))
}

impl CellSource {
    /// Build the source named `kind` from its dictionary (the `regions`
    /// entry of `setFieldsDict`, or an action of `topoSetDict`). `None` when
    /// `kind` is not one of the three sources (the caller reports it).
    ///
    /// # Errors
    /// [`MeshError::DictParse`] for a missing/malformed entry, a negative
    /// radius, or a `coordinateSystem`/`transform` entry (not ported).
    pub fn from_dict(kind: &str, dict: &FoamDict) -> Result<Option<Self>, MeshError> {
        let short = kind.trim_end_matches("ToCell");
        if !matches!(short, "box" | "sphere" | "cylinder") || !kind.ends_with("ToCell") {
            return Ok(None);
        }
        for k in ["coordinateSystem", "transform"] {
            if dict.get(k).is_some() {
                return Err(MeshError::NotImplemented(format!(
                    "{kind}: `{k}` (topoSetSource coordinate transform) is not ported"
                )));
            }
        }
        match short {
            "box" => Self::box_from_dict(dict).map(Some),
            "sphere" => {
                let origin = dict_vector(dict, &["origin", "centre"])
                    .ok_or_else(|| missing(kind, "origin"))?;
                let radius = dict_scalar(dict, &["radius"]).ok_or_else(|| missing(kind, "radius"))?;
                let inner_radius = dict_scalar(dict, &["innerRadius"]).unwrap_or(0.0);
                check_radii(kind, radius, inner_radius)?;
                Ok(Some(CellSource::Sphere {
                    origin,
                    radius,
                    inner_radius,
                }))
            }
            _ => {
                let point1 = dict_vector(dict, &["point1", "p1"])
                    .ok_or_else(|| missing(kind, "point1"))?;
                let point2 = dict_vector(dict, &["point2", "p2"])
                    .ok_or_else(|| missing(kind, "point2"))?;
                let radius = dict_scalar(dict, &["radius", "outerRadius"])
                    .ok_or_else(|| missing(kind, "radius"))?;
                let inner_radius = dict_scalar(dict, &["innerRadius"]).unwrap_or(0.0);
                check_radii(kind, radius, inner_radius)?;
                Ok(Some(CellSource::Cylinder {
                    point1,
                    point2,
                    radius,
                    inner_radius,
                }))
            }
        }
    }

    /// The `boxToCell(mesh, dict)` constructor: `boxes`, else `box`, else
    /// `min` with `max` or `span` (`max = min + span`).
    fn box_from_dict(dict: &FoamDict) -> Result<Self, MeshError> {
        if let Some(FoamEntry::List(items)) = dict.get("boxes") {
            let mut boxes = Vec::new();
            for it in items {
                match it {
                    FoamValue::List(pair) => match pair.as_slice() {
                        [a, b] => match (a.as_vector3(), b.as_vector3()) {
                            (Some(min), Some(max)) => boxes.push(BoundBox { min, max }),
                            _ => return Err(missing("boxToCell", "boxes")),
                        },
                        _ => return Err(missing("boxToCell", "boxes")),
                    },
                    _ => return Err(missing("boxToCell", "boxes")),
                }
            }
            return Ok(CellSource::Box { boxes });
        }
        if let Some(e) = dict.get("box") {
            let v = entry_vectors(e).ok_or_else(|| missing("boxToCell", "box"))?;
            if let [min, max] = v.as_slice() {
                return Ok(CellSource::Box {
                    boxes: vec![BoundBox {
                        min: *min,
                        max: *max,
                    }],
                });
            }
            return Err(missing("boxToCell", "box"));
        }
        let min = dict_vector(dict, &["min"]).ok_or_else(|| missing("boxToCell", "min"))?;
        let max = match dict_vector(dict, &["span"]) {
            // `bb.max() += bb.min()` after reading span into max.
            Some(span) => span + min,
            None => dict_vector(dict, &["max"]).ok_or_else(|| missing("boxToCell", "max"))?,
        };
        Ok(CellSource::Box {
            boxes: vec![BoundBox { min, max }],
        })
    }

    /// Apply the source to `set` over the cell centres `centres`
    /// (`applyToSet` -> `combine`): `New`/`Add` insert each selected cell,
    /// `Subtract` removes it. `New` does **not** clear the set here, as
    /// upstream (the caller hands in a freshly made set).
    pub fn apply(&self, action: SetAction, centres: &[Vector3], set: &mut BTreeSet<usize>) {
        let add = !matches!(action, SetAction::Subtract);
        for (elemi, &c) in centres.iter().enumerate() {
            if self.selects(c) {
                if add {
                    set.insert(elemi);
                } else {
                    set.remove(&elemi);
                }
            }
        }
    }

    /// Whether the cell centre `c` is selected (the body of `combine`).
    pub fn selects(&self, c: Vector3) -> bool {
        match self {
            CellSource::Box { boxes } => boxes.iter().any(|b| b.contains(c)),
            CellSource::Sphere {
                origin,
                radius,
                inner_radius,
            } => {
                let orad2 = radius * radius;
                let irad2 = if *inner_radius > 0.0 {
                    inner_radius * inner_radius
                } else {
                    -1.0
                };
                let d2 = (c - *origin).mag_sqr();
                d2 < orad2 && d2 > irad2
            }
            CellSource::Cylinder {
                point1,
                point2,
                radius,
                inner_radius,
            } => {
                let axis = *point2 - *point1;
                let mag_axis2 = axis.mag_sqr();
                let orad2 = radius * radius;
                let irad2 = if *inner_radius > 0.0 {
                    inner_radius * inner_radius
                } else {
                    -1.0
                };
                let d = c - *point1;
                let mag_d = d.dot(axis);
                if mag_d > 0.0 && mag_d < mag_axis2 {
                    let d2 = d.dot(d) - (mag_d * mag_d) / mag_axis2;
                    d2 < orad2 && d2 > irad2
                } else {
                    false
                }
            }
        }
    }
}

/// `getCheck(.., ge(0))` on the radii and the cylinder's
/// `innerRadius > radius` fatal error.
fn check_radii(kind: &str, radius: f64, inner: f64) -> Result<(), MeshError> {
    if radius < 0.0 || inner < 0.0 {
        return Err(MeshError::DictParse(format!(
            "{kind}: radius and innerRadius must be >= 0"
        )));
    }
    if kind == "cylinderToCell" && inner > radius {
        return Err(MeshError::DictParse(format!(
            "{kind}: inner radius = {inner} cannot be larger than outer radius = {radius}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::dict::FoamFile;

    fn v(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3::new(x, y, z)
    }

    fn dict(text: &str) -> FoamDict {
        FoamFile::parse(text).expect("parse").dict
    }

    #[test]
    fn box_forms_all_parse_to_the_same_box() {
        let a = CellSource::from_dict("boxToCell", &dict("box (0 0 0) (1 2 3);")).unwrap();
        let b = CellSource::from_dict("boxToCell", &dict("min (0 0 0); max (1 2 3);")).unwrap();
        let c = CellSource::from_dict("boxToCell", &dict("min (0 0 0); span (1 2 3);")).unwrap();
        let d = CellSource::from_dict("boxToCell", &dict("boxes (((0 0 0) (1 2 3)));")).unwrap();
        assert!(a.is_some());
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, d);
    }

    #[test]
    fn box_bounds_are_inclusive_and_sphere_cylinder_bounds_exclusive() {
        let bx = CellSource::from_dict("boxToCell", &dict("box (0 0 0) (1 1 1);"))
            .unwrap()
            .unwrap();
        assert!(bx.selects(v(1.0, 0.0, 0.5)));
        assert!(!bx.selects(v(1.0000001, 0.0, 0.5)));
        let sp = CellSource::from_dict(
            "sphereToCell",
            &dict("origin (0 0 0); radius 1; innerRadius 0.5;"),
        )
        .unwrap()
        .unwrap();
        assert!(!sp.selects(v(1.0, 0.0, 0.0)));
        assert!(!sp.selects(v(0.5, 0.0, 0.0)));
        assert!(sp.selects(v(0.75, 0.0, 0.0)));
        let cy = CellSource::from_dict(
            "cylinderToCell",
            &dict("p1 (0 0 0); p2 (0 0 2); outerRadius 1;"),
        )
        .unwrap()
        .unwrap();
        assert!(cy.selects(v(0.5, 0.0, 1.0)));
        assert!(!cy.selects(v(0.5, 0.0, 0.0))); // axial bound is exclusive at 0
        assert!(!cy.selects(v(1.5, 0.0, 1.0)));
    }

    #[test]
    fn apply_new_add_subtract_and_unknown_kind() {
        let src = CellSource::from_dict("boxToCell", &dict("box (0 0 0) (1 1 1);"))
            .unwrap()
            .unwrap();
        let ctrs = [v(0.5, 0.5, 0.5), v(2.0, 0.5, 0.5), v(0.1, 0.1, 0.1)];
        let mut set = BTreeSet::new();
        src.apply(SetAction::New, &ctrs, &mut set);
        assert_eq!(set.iter().copied().collect::<Vec<_>>(), vec![0, 2]);
        src.apply(SetAction::Subtract, &ctrs, &mut set);
        assert!(set.is_empty());
        assert!(CellSource::from_dict("patchToFace", &dict("patch x;")).unwrap().is_none());
        assert!(CellSource::from_dict("boxToCell", &dict("box (0 0 0) (1 1 1); transform {}")).is_err());
        assert!(CellSource::from_dict("cylinderToCell", &dict("p1 (0 0 0); p2 (0 0 1); radius 1; innerRadius 2;")).is_err());
    }
}
