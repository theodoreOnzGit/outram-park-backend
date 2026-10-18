//! **Lowering a stacked hexagonal bed** (`Lattice::hex_bed`, tutorial 8 on
//! #943): the checks, and the CSG it becomes.
//!
//! # What it lowers to
//!
//! `nee_soon::htr10_rmc::core_model::assemble_explicit_triso_with` builds
//! this bed (gh:#472), and this follows its structure with the beginner
//! shapes in place of its hand-written cells:
//!
//! - **the two kinds of ball**, fuel and moderator, are each lowered **once**,
//!   to a universe centred on its own origin whose background cell has no
//!   surface of its own (as a lattice tile's does, [`Lowering::lower_tile_pooled`]);
//!   a fuel ball is a shell around a delta-tracked fuel zone (tutorial 5),
//!   with its TRISO lattice built once, not once per ball;
//! - **one sphere surface per piece site** of the tile (the 13 balls it owns
//!   and the pieces it draws of its neighbours', 23 for Şeker & Çolak's
//!   cell), in tile-local coordinates, shared by every tile;
//! - **one tile universe per distinct state of those pieces** (absent,
//!   moderator or fuel, per piece): its cells are `inside(site sphere)`
//!   filled with the ball universe translated to the site, and the
//!   background outside every kept piece. The all-absent tile is first; it
//!   is also what stands beyond the lattice;
//! - **a CSG `HexLattice`**, one placeholder universe everywhere, then each
//!   tile `(a, b, level)` set to its variant, as `nee_soon` does;
//! - **the container** (a hopper) is the lattice's fill cell.
//!
//! The volumes of the tile backgrounds are counted in one pool (the
//! hopper's volume less every kept ball), as a packed sphere's matrix is.

use super::bed::{BedLayout, PieceGeom};
use super::census::Arrangement;
use super::lattice::Lattice;
use super::lower::{combine, half, Lowered, Lowering};
use super::lower_packed::Pool;
use super::masses::AtomicMasses;
use super::model::{Model, ModelError};
use super::shape::{Cuboid, Shape, Solid};
use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::geometry::lattice::{HexLattice, HexOrientation, Lattice as CsgLattice};
use crate::geometry::position::Position;
use crate::geometry::surface::{BoundaryType, Sphere as CsgSphere, SurfaceKind};
use crate::geometry::universe::Universe;
use std::collections::HashMap;
use uom::si::f64::Length;
use uom::si::length::centimeter;

fn bad<T>(m: impl Into<String>) -> Result<T, ModelError> {
    Err(ModelError::BadValue(m.into()))
}

/// The centre of a ball shape (a sphere), or an error.
fn ball_sphere(s: &Shape, what: &str) -> Result<([f64; 3], f64), ModelError> {
    match s.solid {
        Solid::Sphere(sp) => Ok((sp.centre_cm(), sp.radius_cm())),
        _ => bad(format!(
            "{what} must be a sphere, not {}",
            s.solid.describe()
        )),
    }
}

/// **A stacked bed must be well formed**: a hopper holding it, a tile of
/// identical balls on a hexagonal lattice with faces normal to y, a
/// moderator ball of the same radius, a fuel fraction between 0 and 1, a
/// loading height inside the hopper, and the tile's background the
/// container's material (a rejected ball's place becomes it).
pub(crate) fn check_hex_bed(model: &Model, s: &Shape, lattice: &Lattice) -> Result<(), ModelError> {
    let Solid::Hopper(hopper) = s.solid else {
        return bad(format!(
            "a stacked hexagonal bed (Lattice::hex_bed) fills a Hopper, not {}",
            s.solid.describe()
        ));
    };
    let spec = lattice
        .bed_spec()
        .ok_or_else(|| ModelError::BadValue("not a bed (a defect)".into()))?;
    let Some(prism) = lattice.hex_prism() else {
        return bad("a hexagonal bed's tile must be a HexPrism");
    };
    let tile = &lattice.tiles()[0];
    model.check_shape_with(tile, false, true, true)?;
    if prism.faces_kind() != super::hex::HexFaces::NormalToY {
        return bad(
            "a stacked hexagonal bed is laid out for tiles with faces normal to y \
             (HexFaces::NormalToY, Şeker & Çolak's cell)",
        );
    }
    if s.boundary.is_some() {
        return bad(
            "the hopper of a bed holds the bed inside the model: it has no boundary of its own",
        );
    }
    if tile.material != s.material {
        return bad(format!(
            "a bed's tile is filled with the hopper's material ({}), not {}: the place of a \
             rejected ball becomes the hopper's material",
            s.material.name(),
            tile.material.name()
        ));
    }
    if !(spec.fuel_fraction.is_finite() && (0.0..=1.0).contains(&spec.fuel_fraction)) {
        return bad(format!(
            "the fuel fraction of a bed is a share between 0 and 1, not {}",
            spec.fuel_fraction
        ));
    }
    if !(spec.bed_top_cm.is_finite()
        && spec.bed_top_cm > hopper.floor_z_cm()
        && spec.bed_top_cm <= hopper.top_z_cm())
    {
        return bad(format!(
            "the bed is loaded to {} cm, which must lie between the hopper's floor ({} cm) and \
             its top ({} cm)",
            spec.bed_top_cm,
            hopper.floor_z_cm(),
            hopper.top_z_cm()
        ));
    }
    let Some(first) = tile.children.first() else {
        return bad("a bed's tile holds its fuel balls: give it at least one with .containing(..)");
    };
    let (c0, r0) = ball_sphere(first, "a ball of a bed's tile")?;
    let neg = |c: [f64; 3]| [-c[0], -c[1], -c[2]];
    let at_origin = first.translated(neg(c0));
    for (i, c) in tile.children.iter().enumerate() {
        let (ci, _) = ball_sphere(c, "a ball of a bed's tile")?;
        if c.translated(neg(ci)) != at_origin {
            return bad(format!(
                "every ball of a bed's tile is the same fuel ball at its own site; ball {i} differs \
                 from ball 0 (the moderator ball is given to Lattice::hex_bed separately)"
            ));
        }
    }
    let moderator = lattice
        .bed_moderator()
        .ok_or_else(|| ModelError::BadValue("a bed has a moderator ball (a defect)".into()))?;
    let (cm, rm) = ball_sphere(moderator, "the moderator ball")?;
    if (rm - r0).abs() > 1.0e-12 * r0 {
        return bad(format!(
            "the moderator ball must have the fuel ball's radius ({r0} cm), not {rm} cm: both \
             stand at the same sites"
        ));
    }
    if moderator.lattice.is_some() || moderator.dh.is_some() {
        return bad("the moderator ball holds shapes, not a lattice or a DhRegion of its own");
    }
    model.check_shape(&moderator.translated(neg(cm)), false, true)?;
    let (_, images) = lattice.hex_pieces().map_err(ModelError::BadValue)?;
    let mut pieces: Vec<Shape> = tile.children.clone();
    pieces.extend(images.into_iter().map(|(shape, _)| shape));
    super::model::check_disjoint(&pieces)?;
    Ok(())
}

impl Lowering {
    /// **Lower a stacked bed in its hopper** (see the module docs). Checked
    /// by [`check_hex_bed`] first.
    pub(crate) fn lower_hex_bed(
        &mut self,
        s: &Shape,
        lattice: &Lattice,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        let Solid::Hopper(hopper) = s.solid else {
            return bad("a bed fills a hopper (a defect: checked before lowering)");
        };
        let spec = lattice
            .bed_spec()
            .ok_or_else(|| ModelError::BadValue("not a bed (a defect)".into()))?
            .clone();
        let prism = lattice
            .hex_prism()
            .ok_or_else(|| ModelError::BadValue("a bed's tile must be a HexPrism".into()))?;
        let tile = &lattice.tiles()[0];
        let (owned, images) = lattice.hex_pieces().map_err(ModelError::BadValue)?;
        let mut pieces: Vec<PieceGeom> = owned.clone();
        pieces.extend(images.iter().map(|(_, p)| *p));
        let (c0, r) = ball_sphere(&tile.children[0], "a ball")?;
        let neg = |c: [f64; 3]| [-c[0], -c[1], -c[2]];
        let fuel_ball = tile.children[0].translated(neg(c0));
        let moderator = lattice
            .bed_moderator()
            .ok_or_else(|| ModelError::BadValue("a bed has a moderator ball (a defect)".into()))?;
        let (cm, _) = ball_sphere(moderator, "the moderator ball")?;
        let moderator_ball = moderator.translated(neg(cm));

        let sites: Vec<[f64; 3]> = owned.iter().map(|p| p.local).collect();
        let layout = BedLayout::compute(
            hopper,
            sites,
            r,
            prism.pitch_cm(),
            prism.height_cm(),
            spec.fuel_fraction,
            spec.bed_top_cm,
        );
        if !layout.lattice_covers_container() {
            return bad(
                "the lattice does not cover the hopper: a kept ball is owned by a tile beyond it \
                 (a defect)",
            );
        }

        // The two kinds of ball, each once.
        let was = (self.in_tile, self.tile_has_heavy_metal);
        self.in_tile = true;
        self.tile_has_heavy_metal = false;
        let fuel = self.lower_tile_pooled(&fuel_ball, t, masses, None);
        let fuel_cell = self.cells.len().saturating_sub(1);
        let fuel_holds_heavy_metal = self.tile_has_heavy_metal;
        let moderator = fuel
            .is_ok()
            .then(|| self.lower_tile_pooled(&moderator_ball, t, masses, None));
        let moderator_cell = self.cells.len().saturating_sub(1);
        self.in_tile = was.0;
        self.tile_has_heavy_metal = was.1;
        let (fuel_universe, _) = fuel?;
        let (moderator_universe, _) = moderator.ok_or_else(|| {
            ModelError::BadValue("the moderator ball was not lowered (a defect)".into())
        })??;

        // One sphere per piece site, in tile-local coordinates.
        let first_site = self.surfaces.len();
        for p in &pieces {
            self.surfaces.push(SurfaceKind::Sphere(CsgSphere {
                x0: p.local[0],
                y0: p.local[1],
                z0: p.local[2],
                r,
                bc: BoundaryType::Transmissive,
            }));
        }

        // The pool that counts the tile backgrounds' volume.
        let m = self.material_index(&tile.material, t, masses)?;
        let ball_v = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
        let tile_v = prism.volume_cm3();
        let hopper_v = hopper.volume_cm3();
        let pool = self.pools.len();
        self.pools.push(Pool {
            fill_cell: usize::MAX,
            material: m,
            component: tile.component,
            volume_cm3: hopper_v - layout.census.loaded_balls as f64 * ball_v,
            container_volume_cm3: hopper_v,
            particle_cells: vec![(fuel_cell, ball_v), (moderator_cell, ball_v)],
            arrangement: Arrangement::HexBed {
                pitch_cm: prism.pitch_cm(),
                height_cm: prism.height_cm(),
                rings: layout.n_rings,
                levels: layout.n_levels,
            },
            requested: owned.len() as f64 * ball_v / tile_v,
            delta_tracked: false,
        });

        // One universe per distinct state of the pieces, in order of first
        // use; the all-absent one first.
        let mut variants: HashMap<Vec<u8>, usize> = HashMap::new();
        let mut order: Vec<Vec<u8>> = Vec::new();
        let nr = layout.n_rings as i32;
        let mut keys: Vec<(i32, i32, i32, Vec<u8>)> = Vec::new();
        order.push(vec![0; pieces.len()]);
        for level in 0..layout.n_levels as i32 {
            for a in -(nr - 1)..=(nr - 1) {
                for b in -(nr - 1)..=(nr - 1) {
                    if super::bed::hex_ring(a, b) > layout.n_rings - 1 {
                        continue;
                    }
                    let key: Vec<u8> = pieces
                        .iter()
                        .map(|p| layout.piece_state(a, b, level, p))
                        .collect();
                    if !order.contains(&key) {
                        order.push(key.clone());
                    }
                    keys.push((a, b, level, key));
                }
            }
        }
        for key in &order {
            let u = self.variant_universe(
                key,
                first_site,
                &pieces,
                [moderator_universe, fuel_universe],
                tile,
                pool,
                t,
                masses,
            )?;
            variants.insert(key.clone(), u);
        }
        let outer_tile = variants[&order[0]];

        // The container, the outer universe (never reached: the lattice
        // covers the hopper), the lattice and the fill cell.
        let (inside, outside, surfaces) = self.solid_surfaces(&s.solid);
        let mat = self.material_index(&s.material, t, masses)?;
        let everywhere = vec![
            half(surfaces[0], HalfSpaceSense::Inside),
            half(surfaces[0], HalfSpaceSense::Outside),
            RegionToken::Union,
        ];
        let mut outer_cells = Vec::new();
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, everywhere, mat, t),
            &mut outer_cells,
            s.component,
            Some(0.0),
        );
        self.universes.push(Universe {
            id: self.universes.len() as i32,
            cell_indices: outer_cells,
        });
        let outer = self.universes.len() - 1;

        let placeholder: Vec<Vec<Vec<usize>>> = (0..layout.n_levels)
            .map(|_| {
                (0..layout.n_rings)
                    .rev()
                    .map(|r| vec![outer_tile; if r == 0 { 1 } else { 6 * r }])
                    .collect()
            })
            .collect();
        let mut csg = HexLattice::from_rings_3d(
            self.lattices.len() as i32 + 1,
            HexOrientation::Y,
            Position::new(0.0, 0.0, layout.lattice_centre_z()),
            prism.pitch_cm(),
            prism.height_cm(),
            &placeholder,
            Some(outer),
        );
        for (a, b, level, key) in &keys {
            let i = [a + nr - 1, b + nr - 1, *level];
            debug_assert!(csg.are_valid_indices(i));
            let flat = csg.flat_index(i);
            csg.universes[flat] = variants[key] as i32;
        }
        self.lattices.push(CsgLattice::Hex(csg));
        let li = self.lattices.len() - 1;

        if fuel_holds_heavy_metal {
            // Tile positions are local; the source starts over the bed.
            let r_bed = hopper.radius_cm();
            let cm = Length::new::<centimeter>;
            let region = Cuboid::sides(
                cm(2.0 * r_bed),
                cm(2.0 * r_bed),
                cm(spec.bed_top_cm - hopper.cone_bottom_z_cm()),
            )
            .centred_at(
                cm(0.0),
                cm(0.0),
                cm(0.5 * (spec.bed_top_cm + hopper.cone_bottom_z_cm())),
            );
            self.note_heavy_metal_bounds(&Solid::Cuboid(region));
        }
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::fill(id, inside, CellFill::Lattice(li), Position::ZERO),
            cells_of,
            s.component,
            None,
        );
        self.pools[pool].fill_cell = self.cells.len() - 1;
        self.beds.push(layout.census.clone());
        Ok(Lowered { outside, surfaces })
    }

    /// The universe of one tile state: a fill cell per kept piece and the
    /// background outside them all.
    #[allow(clippy::too_many_arguments)]
    fn variant_universe(
        &mut self,
        key: &[u8],
        first_site: usize,
        pieces: &[PieceGeom],
        ball_universes: [usize; 2],
        tile: &Shape,
        pool: usize,
        t: f64,
        masses: &AtomicMasses,
    ) -> Result<usize, ModelError> {
        let mut cells_of = Vec::new();
        let mut children: Vec<Vec<RegionToken>> = Vec::new();
        for (i, (&state, p)) in key.iter().zip(pieces).enumerate() {
            if state == 0 {
                continue;
            }
            let id = self.cells.len() as i32 + 1;
            self.push_cell(
                Cell::fill(
                    id,
                    vec![half(first_site + i, HalfSpaceSense::Inside)],
                    CellFill::Universe(ball_universes[usize::from(state) - 1]),
                    Position::new(p.local[0], p.local[1], p.local[2]),
                ),
                &mut cells_of,
                None,
                None,
            );
            children.push(vec![half(first_site + i, HalfSpaceSense::Outside)]);
        }
        let region = if children.is_empty() {
            self.everywhere()
        } else {
            combine(&[], &children)
        };
        let m = self.material_index(&tile.material, t, masses)?;
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, region, m, t),
            &mut cells_of,
            tile.component,
            None,
        );
        let last = self.cells.len() - 1;
        self.cell_pools[last] = Some(pool);
        self.universes.push(Universe {
            id: self.universes.len() as i32,
            cell_indices: cells_of,
        });
        Ok(self.universes.len() - 1)
    }
}
