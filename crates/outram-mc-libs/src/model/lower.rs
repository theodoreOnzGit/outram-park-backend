//! **Lowering shapes to CSG** (gh #938, #926): what the user never sees.
//!
//! Shapes are visited inside out (each shape after the shapes it contains).
//! Each shape is its own surfaces (one sphere, or six planes for a box) and
//! one cell: `inside(self)` minus, per contained shape, `outside(child)`, in
//! the token order the crate's concentric builders use (`outside(child),
//! inside(self), ∩, outside(child 2), ∩, ...`; `fhr_pebble_geometry`,
//! `build_triso_particle`). A box's inside is `+x0 -x1 ∩ +y0 ∩ -y1 ∩ +z0 ∩
//! -z1 ∩`, as `pebble_beds::fhr_pebble::homogeneous_cube` writes it; its
//! outside is that followed by `Complement`.
//!
//! **A lattice** (#926) lowers each distinct tile once, to a universe whose
//! background cell is `outside(child 1) ∩ outside(child 2) ...` with no
//! surface of its own (the lattice bounds it), exactly as
//! `build_triso_particle` writes its matrix cell. The grid becomes a CSG
//! [`RectLattice`] centred on its container, whose `outer` universe is the
//! container's material, and the container one fill cell. Universes are
//! numbered in creation order and the root comes last.

use super::masses::AtomicMasses;
use super::material::{to_atoms_per_barn_cm, Material};
use super::model::{ModelError, NuclideKey};
use super::resolve::ResolvedMaterial;
use super::shape::{Component, Shape, Solid};
use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::geometry::lattice::{Lattice as CsgLattice, RectLattice};
use crate::geometry::position::Position;
use crate::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use crate::geometry::universe::Universe;
use crate::material::material::NuclideComponent;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// A solid's inside tokens, outside tokens and surface indices.
pub(crate) type SolidTokens = (Vec<RegionToken>, Vec<RegionToken>, Vec<usize>);

/// One lowered shape: the tokens a parent uses to subtract it, and its own
/// surfaces (for the boundary condition of an outermost shape).
pub(crate) struct Lowered {
    pub(crate) outside: Vec<RegionToken>,
    pub(crate) surfaces: Vec<usize>,
}

/// The CSG being built.
#[derive(Default)]
pub(crate) struct Lowering {
    pub(crate) surfaces: Vec<SurfaceKind>,
    pub(crate) cells: Vec<Cell>,
    pub(crate) cell_components: Vec<Option<Component>>,
    /// Per cell: the exact volume of one placed copy \[cm³\]; `None` for a
    /// fill cell.
    pub(crate) cell_volumes: Vec<Option<f64>>,
    /// Per cell: the pool its volume is counted in (the matrix of a sphere
    /// filled with whole particles, whose cells are clipped by the sphere
    /// and so have no exact volume each); `None` otherwise.
    pub(crate) cell_pools: Vec<Option<usize>>,
    /// The pools, one per sphere filled with particles (see
    /// `super::lower_packed`).
    pub(crate) pools: Vec<super::lower_packed::Pool>,
    /// Per delta-tracked region: the material indices it can present (its
    /// majorant must bound them all).
    pub(crate) delta_regions: Vec<Vec<usize>>,
    pub(crate) universes: Vec<Universe>,
    pub(crate) lattices: Vec<CsgLattice>,
    descriptions: Vec<Material>,
    pub(crate) materials: Vec<crate::material::material::Material>,
    pub(crate) resolved: Vec<ResolvedMaterial>,
    pub(crate) nuclides: Vec<NuclideKey>,
    /// Bounds of the shapes holding heavy metal, in the root frame.
    pub(crate) fissile_bounds: Option<([f64; 3], [f64; 3])>,
    /// Per component (by index): its own temperature \[K\], when the model
    /// gives it one (gh #945).
    pub(crate) component_temperatures: Vec<Option<f64>>,
    /// While lowering a lattice tile, positions are tile-local: heavy metal
    /// is noted here instead of in `fissile_bounds`.
    pub(crate) in_tile: bool,
    pub(crate) tile_has_heavy_metal: bool,
    /// While lowering the tiles of a one-layer lattice: the height \[cm\] it
    /// is extruded through (its container's), which a rod and a tile take
    /// for their own when their volume is counted.
    pub(crate) extrusion: Option<f64>,
    far_plane: Option<usize>,
}

pub(crate) fn half(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

/// `inside` minus each child, in the concentric builders' order:
/// `child1, inside, ∩, child2, ∩, ...` (each child given by its outside
/// tokens). With no `inside` (a lattice tile): `child1, child2, ∩, ...`.
pub(crate) fn combine(inside: &[RegionToken], children: &[Vec<RegionToken>]) -> Vec<RegionToken> {
    let mut v = Vec::new();
    match children.split_first() {
        None => v.extend_from_slice(inside),
        Some((first, rest)) => {
            v.extend_from_slice(first);
            if !inside.is_empty() {
                v.extend_from_slice(inside);
                v.push(RegionToken::Intersection);
            }
            for c in rest {
                v.extend_from_slice(c);
                v.push(RegionToken::Intersection);
            }
        }
    }
    v
}

impl Lowering {
    /// Push `solid`'s surfaces (transmissive) and return `(inside tokens,
    /// outside tokens, surface indices)`.
    pub(crate) fn solid_surfaces(&mut self, solid: &Solid) -> SolidTokens {
        let bc = BoundaryType::Transmissive;
        match solid {
            Solid::Sphere(s) => {
                let c = s.centre_cm();
                self.surfaces
                    .push(SurfaceKind::Sphere(crate::geometry::surface::Sphere {
                        x0: c[0],
                        y0: c[1],
                        z0: c[2],
                        r: s.radius_cm(),
                        bc,
                    }));
                let i = self.surfaces.len() - 1;
                (
                    vec![half(i, HalfSpaceSense::Inside)],
                    vec![half(i, HalfSpaceSense::Outside)],
                    vec![i],
                )
            }
            Solid::Cuboid(b) => {
                let (l, u) = b.bounds();
                let i0 = self.surfaces.len();
                self.surfaces.extend([
                    SurfaceKind::XPlane(XPlane { x0: l[0], bc }),
                    SurfaceKind::XPlane(XPlane { x0: u[0], bc }),
                    SurfaceKind::YPlane(YPlane { y0: l[1], bc }),
                    SurfaceKind::YPlane(YPlane { y0: u[1], bc }),
                    SurfaceKind::ZPlane(ZPlane { z0: l[2], bc }),
                    SurfaceKind::ZPlane(ZPlane { z0: u[2], bc }),
                ]);
                let (o, i) = (HalfSpaceSense::Outside, HalfSpaceSense::Inside);
                let mut inside = vec![half(i0, o), half(i0 + 1, i), RegionToken::Intersection];
                for (k, sense) in [(2, o), (3, i), (4, o), (5, i)] {
                    inside.push(half(i0 + k, sense));
                    inside.push(RegionToken::Intersection);
                }
                let mut outside = inside.clone();
                outside.push(RegionToken::Complement);
                (inside, outside, (i0..i0 + 6).collect())
            }
            Solid::Cylinder(c) => {
                let ctr = c.centre_cm();
                let cylinder = SurfaceKind::ZCylinder(ZCylinder {
                    x0: ctr[0],
                    y0: ctr[1],
                    r: c.radius_cm(),
                    bc,
                });
                if c.is_rod() {
                    self.surfaces.push(cylinder);
                    let i = self.surfaces.len() - 1;
                    return (
                        vec![half(i, HalfSpaceSense::Inside)],
                        vec![half(i, HalfSpaceSense::Outside)],
                        vec![i],
                    );
                }
                // `+z0 ∩ -z1 ∩ -cylinder`, as an OpenMC card writes it.
                let (lo, hi) = c.z_range();
                let i0 = self.surfaces.len();
                self.surfaces.extend([
                    SurfaceKind::ZPlane(ZPlane { z0: lo, bc }),
                    SurfaceKind::ZPlane(ZPlane { z0: hi, bc }),
                    cylinder,
                ]);
                let (o, i) = (HalfSpaceSense::Outside, HalfSpaceSense::Inside);
                let mut inside = vec![half(i0, o), half(i0 + 1, i), RegionToken::Intersection];
                inside.push(half(i0 + 2, i));
                inside.push(RegionToken::Intersection);
                let mut outside = inside.clone();
                outside.push(RegionToken::Complement);
                (inside, outside, (i0..i0 + 3).collect())
            }
        }
    }

    /// A region holding every point: `-p ∪ +p` on a transmissive plane far
    /// outside any model (`x = 1e30` cm), made once. For an empty lattice
    /// tile, whose background has no child to be the outside of.
    pub(crate) fn everywhere(&mut self) -> Vec<RegionToken> {
        let p = match self.far_plane {
            Some(p) => p,
            None => {
                self.surfaces.push(SurfaceKind::XPlane(XPlane {
                    x0: 1.0e30,
                    bc: BoundaryType::Transmissive,
                }));
                self.far_plane = Some(self.surfaces.len() - 1);
                self.surfaces.len() - 1
            }
        };
        vec![
            half(p, HalfSpaceSense::Inside),
            half(p, HalfSpaceSense::Outside),
            RegionToken::Union,
        ]
    }

    pub(crate) fn push_cell(
        &mut self,
        cell: Cell,
        cells_of: &mut Vec<usize>,
        component: Option<Component>,
        volume: Option<f64>,
    ) {
        cells_of.push(self.cells.len());
        self.cells.push(cell);
        self.cell_components.push(component);
        self.cell_volumes.push(volume);
        self.cell_pools.push(None);
    }

    fn note_heavy_metal(&mut self, m: usize, solid: &Solid) {
        if !self.resolved[m]
            .nuclides
            .iter()
            .any(|(i, _)| i.is_heavy_metal())
        {
            return;
        }
        self.note_heavy_metal_bounds(solid);
    }

    /// **Lower `s` and everything inside it** into the universe whose cell
    /// list is `cells_of`.
    pub(crate) fn lower(
        &mut self,
        s: &Shape,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        let t = self.temperature_of(s, t);
        if let Some(spec) = &s.dh {
            return self.lower_dh(s, spec, t, masses, cells_of);
        }
        if let Some(lattice) = &s.lattice {
            if let Solid::Sphere(zone) = s.solid {
                return self.lower_lattice_in_sphere(s, zone, lattice, t, masses, cells_of);
            }
            return self.lower_lattice_container(s, lattice, t, masses, cells_of);
        }
        let children: Vec<Vec<RegionToken>> = s
            .children
            .iter()
            .map(|c| self.lower(c, t, masses, cells_of).map(|l| l.outside))
            .collect::<Result<_, _>>()?;
        let (inside, outside, surfaces) = self.solid_surfaces(&s.solid);
        let m = self.material_index(&s.material, t, masses)?;
        self.note_heavy_metal(m, &s.solid);
        let volume = s.solid.volume_within(self.extrusion)
            - s.children
                .iter()
                .map(|c| c.solid.volume_within(self.extrusion))
                .sum::<f64>();
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, combine(&inside, &children), m, t),
            cells_of,
            s.component,
            Some(volume),
        );
        Ok(Lowered { outside, surfaces })
    }

    /// One lattice tile: a universe of its children and its background, the
    /// background bounded only by the lattice. Returns the universe index.
    pub(crate) fn lower_tile(
        &mut self,
        tile: &Shape,
        t: f64,
        masses: &AtomicMasses,
    ) -> Result<usize, ModelError> {
        self.lower_tile_pooled(tile, t, masses, None)
            .map(|(u, _)| u)
    }

    /// [`Self::lower_tile`], with the background's volume counted in `pool`
    /// when given (a tile clipped by a sphere). Also returns each child's
    /// own cell (its outermost layer) and the child's volume.
    ///
    /// **A tile that is itself filled with a lattice** (an assembly of pins
    /// in a core of assemblies) is a universe of one fill cell, bounded by
    /// the enclosing lattice like any tile's background: no surface of its
    /// own, so no second boundary coincides with the lattice's.
    pub(crate) fn lower_tile_pooled(
        &mut self,
        tile: &Shape,
        t: f64,
        masses: &AtomicMasses,
        pool: Option<usize>,
    ) -> Result<(usize, Vec<(usize, f64)>), ModelError> {
        let t = self.temperature_of(tile, t);
        let mut cells_of = Vec::new();
        if let Some(lattice) = &tile.lattice {
            let region = self.everywhere();
            let tile_volume = match (self.extrusion, tile.solid) {
                (Some(h), Solid::Cuboid(b)) => b.sides_cm()[0] * b.sides_cm()[1] * h,
                _ => tile.solid.volume_cm3(),
            };
            let (li, held, _) = self.lower_grid(
                lattice,
                &tile.material,
                tile.component,
                tile.solid.centre_cm(),
                None,
                tile_volume,
                None,
                t,
                masses,
            )?;
            self.tile_has_heavy_metal |= held;
            let id = self.cells.len() as i32 + 1;
            self.push_cell(
                Cell::fill(id, region, CellFill::Lattice(li), Position::ZERO),
                &mut cells_of,
                tile.component,
                None,
            );
            self.universes.push(Universe {
                id: self.universes.len() as i32,
                cell_indices: cells_of,
            });
            return Ok((self.universes.len() - 1, Vec::new()));
        }
        let mut child_cells = Vec::new();
        let mut children: Vec<Vec<RegionToken>> = Vec::new();
        for c in &tile.children {
            children.push(self.lower(c, t, masses, &mut cells_of)?.outside);
            child_cells.push((self.cells.len() - 1, c.solid.volume_cm3()));
        }
        let region = if children.is_empty() {
            self.everywhere()
        } else {
            combine(&[], &children)
        };
        let m = self.material_index(&tile.material, t, masses)?;
        self.note_heavy_metal(m, &tile.solid);
        let tile_volume = match (self.extrusion, tile.solid) {
            (Some(h), Solid::Cuboid(b)) => b.sides_cm()[0] * b.sides_cm()[1] * h,
            _ => tile.solid.volume_cm3(),
        };
        let volume = tile_volume
            - tile
                .children
                .iter()
                .map(|c| c.solid.volume_within(self.extrusion))
                .sum::<f64>();
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, region, m, t),
            &mut cells_of,
            tile.component,
            pool.is_none().then_some(volume),
        );
        if let Some(p) = self.cell_pools.last_mut() {
            *p = pool;
        }
        self.universes.push(Universe {
            id: self.universes.len() as i32,
            cell_indices: cells_of,
        });
        Ok((self.universes.len() - 1, child_cells))
    }

    /// **The grid of a lattice**: each distinct tile lowered once, then (if
    /// `container` is given, the shape the lattice fills) the container's
    /// surfaces, the `outer` universe of the container's material, and the
    /// CSG [`RectLattice`] centred on `centre`. Returns the lattice's
    /// index, whether any tile holds heavy metal, and the container's
    /// lowering when there was one.
    ///
    /// A **one-layer** lattice (`n_z = 1`) is extruded: the CSG lattice
    /// ignores z for it, so its tiles run through the whole height of what
    /// holds them. `extruded_through` is that height (the container's) for
    /// counting volumes, or `None` to count each tile at its own height.
    #[allow(clippy::too_many_arguments)]
    fn lower_grid(
        &mut self,
        lattice: &super::lattice::Lattice,
        material: &Material,
        component: Option<Component>,
        centre: [f64; 3],
        container: Option<&Solid>,
        container_volume: f64,
        extruded_through: Option<f64>,
        t: f64,
        masses: &AtomicMasses,
    ) -> Result<(usize, bool, Option<SolidTokens>), ModelError> {
        // Checked by `Model::check_shape` before lowering.
        let pitch = lattice
            .pitch_cm()
            .ok_or_else(|| ModelError::BadValue("a lattice tile must be a box".into()))?;
        let n = lattice.dimensions();
        let was = (self.in_tile, self.tile_has_heavy_metal, self.extrusion);
        self.in_tile = true;
        self.tile_has_heavy_metal = false;
        if n[2] == 1 {
            self.extrusion = extruded_through.or(self.extrusion);
        }
        let tile_universes: Result<Vec<usize>, ModelError> = lattice
            .tiles()
            .iter()
            .map(|tile| self.lower_tile(tile, t, masses))
            .collect();
        let tiles_hold_heavy_metal = self.tile_has_heavy_metal;
        let tile_universes = tile_universes?;
        let extrusion = self.extrusion;
        self.in_tile = was.0;
        self.tile_has_heavy_metal = was.1;
        self.extrusion = was.2;

        let lowered = container.map(|c| self.solid_surfaces(c));
        // The outer universe: the container's material wherever the grid
        // does not reach. `-s ∪ +s` on the container's first surface (or
        // everywhere, for a tile, which the enclosing lattice bounds).
        let m = self.material_index(material, t, masses)?;
        let grid_volume = n[0] as f64
            * pitch[0]
            * n[1] as f64
            * pitch[1]
            * extrusion.unwrap_or(n[2] as f64 * pitch[2]);
        let mut outer_cells = Vec::new();
        // A grid that covers its container leaves no outer.
        let outer_volume = Some((container_volume - grid_volume).max(0.0));
        let everywhere = match &lowered {
            Some((_, _, surfaces)) => vec![
                half(surfaces[0], HalfSpaceSense::Inside),
                half(surfaces[0], HalfSpaceSense::Outside),
                RegionToken::Union,
            ],
            None => self.everywhere(),
        };
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, everywhere, m, t),
            &mut outer_cells,
            component,
            outer_volume,
        );
        self.universes.push(Universe {
            id: self.universes.len() as i32,
            cell_indices: outer_cells,
        });
        let outer = self.universes.len() - 1;

        let (lower, _) = lattice.layout(pitch, centre);
        self.lattices.push(CsgLattice::Rect(RectLattice {
            id: self.lattices.len() as i32 + 1,
            n,
            lower_left: Position::new(lower[0], lower[1], lower[2]),
            pitch,
            universes: lattice
                .pattern()
                .iter()
                .map(|&k| tile_universes[k])
                .collect(),
            outer: Some(outer),
        }));
        let li = self.lattices.len() - 1;
        Ok((li, tiles_hold_heavy_metal, lowered))
    }

    fn lower_lattice_container(
        &mut self,
        s: &Shape,
        lattice: &super::lattice::Lattice,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        // A one-layer lattice is extruded through its container's height.
        let (lo, hi) = s.solid.bounds();
        let height = (hi[2] - lo[2]).is_finite().then_some(hi[2] - lo[2]);
        let (li, tiles_hold_heavy_metal, lowered) = self.lower_grid(
            lattice,
            &s.material,
            s.component,
            s.solid.centre_cm(),
            Some(&s.solid),
            s.solid.volume_cm3(),
            height,
            t,
            masses,
        )?;
        self.tile_has_heavy_metal |= tiles_hold_heavy_metal;
        let (inside, outside, surfaces) = lowered.ok_or_else(|| {
            ModelError::BadValue("a lattice container was not lowered (a defect)".into())
        })?;
        if tiles_hold_heavy_metal {
            // Tile positions are local; the source starts over the container.
            self.note_heavy_metal_bounds(&s.solid);
        }
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::fill(id, inside, CellFill::Lattice(li), Position::ZERO),
            cells_of,
            s.component,
            None,
        );
        Ok(Lowered { outside, surfaces })
    }

    pub(crate) fn note_heavy_metal_bounds(&mut self, solid: &Solid) {
        if self.in_tile {
            self.tile_has_heavy_metal = true;
            return;
        }
        let (l, u) = solid.bounds();
        let b = self.fissile_bounds.get_or_insert((l, u));
        for i in 0..3 {
            b.0[i] = b.0[i].min(l[i]);
            b.1[i] = b.1[i].max(u[i]);
        }
    }

    /// **The temperature \[K\] of shape `s`** whose container is at
    /// `inherited` (gh #945): its own, else its component's, else
    /// `inherited`. Checked positive and finite by `Model::assemble`.
    pub(crate) fn temperature_of(&self, s: &Shape, inherited: f64) -> f64 {
        if let Some(t) = s.temperature {
            return t.kelvin();
        }
        s.component
            .and_then(|c| self.component_temperatures.get(c.index).copied().flatten())
            .unwrap_or(inherited)
    }

    /// The low-level material index of `m` at temperature `t` \[K\],
    /// resolving it on first use (equal materials at the same temperature
    /// share one entry; nuclides are keyed by nuclide, thermal law and
    /// temperature, so each is processed once per temperature it is present
    /// at, gh #945).
    pub(crate) fn material_index(
        &mut self,
        m: &Material,
        t: f64,
        masses: &AtomicMasses,
    ) -> Result<usize, ModelError> {
        if let Some(i) = self
            .descriptions
            .iter()
            .zip(&self.materials)
            .position(|(d, low)| d == m && low.temperature.to_bits() == t.to_bits())
        {
            return Ok(i);
        }
        let r = m.resolve(masses)?;
        let mut components = Vec::with_capacity(r.nuclides.len());
        for &(isotope, d) in &r.nuclides {
            let key = NuclideKey {
                isotope,
                law: r.moderator.and_then(|md| md.law_for(isotope)),
                temperature: ThermodynamicTemperature::new::<kelvin>(t),
            };
            let idx = match self.nuclides.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    self.nuclides.push(key);
                    self.nuclides.len() - 1
                }
            };
            components.push(NuclideComponent {
                nuclide_idx: idx,
                atom_density: to_atoms_per_barn_cm(d),
            });
        }
        self.materials.push(crate::material::material::Material {
            id: self.materials.len() as i32 + 1,
            name: r.name.clone(),
            components,
            temperature: t,
        });
        self.descriptions.push(m.clone());
        self.resolved.push(r);
        Ok(self.materials.len() - 1)
    }
}
