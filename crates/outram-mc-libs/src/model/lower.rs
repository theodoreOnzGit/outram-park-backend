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
use crate::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use crate::geometry::universe::Universe;
use crate::material::material::NuclideComponent;

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
    /// While lowering a lattice tile, positions are tile-local: heavy metal
    /// is noted here instead of in `fissile_bounds`.
    pub(crate) in_tile: bool,
    pub(crate) tile_has_heavy_metal: bool,
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
    pub(crate) fn solid_surfaces(
        &mut self,
        solid: &Solid,
    ) -> (Vec<RegionToken>, Vec<RegionToken>, Vec<usize>) {
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
        let volume =
            s.solid.volume_cm3() - s.children.iter().map(|c| c.solid.volume_cm3()).sum::<f64>();
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
    pub(crate) fn lower_tile_pooled(
        &mut self,
        tile: &Shape,
        t: f64,
        masses: &AtomicMasses,
        pool: Option<usize>,
    ) -> Result<(usize, Vec<(usize, f64)>), ModelError> {
        let mut cells_of = Vec::new();
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
        let volume = tile.solid.volume_cm3()
            - tile
                .children
                .iter()
                .map(|c| c.solid.volume_cm3())
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

    fn lower_lattice_container(
        &mut self,
        s: &Shape,
        lattice: &super::lattice::Lattice,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        // Checked by `Model::check_shape` before lowering.
        let pitch = lattice
            .pitch_cm()
            .ok_or_else(|| ModelError::BadValue("a lattice tile must be a box".into()))?;
        let was = (self.in_tile, self.tile_has_heavy_metal);
        self.in_tile = true;
        self.tile_has_heavy_metal = false;
        let tile_universes: Vec<usize> = lattice
            .tiles()
            .iter()
            .map(|tile| self.lower_tile(tile, t, masses))
            .collect::<Result<_, _>>()?;
        let tiles_hold_heavy_metal = self.tile_has_heavy_metal;
        self.in_tile = was.0;
        self.tile_has_heavy_metal = was.1 || tiles_hold_heavy_metal;

        let (inside, outside, surfaces) = self.solid_surfaces(&s.solid);
        // The outer universe: the container's material wherever the grid
        // does not reach. `-s ∪ +s` on the container's first surface.
        let m = self.material_index(&s.material, t, masses)?;
        let n = lattice.dimensions();
        let grid_volume = (0..3).map(|i| n[i] as f64 * pitch[i]).product::<f64>();
        let mut outer_cells = Vec::new();
        let everywhere = vec![
            half(surfaces[0], HalfSpaceSense::Inside),
            half(surfaces[0], HalfSpaceSense::Outside),
            RegionToken::Union,
        ];
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, everywhere, m, t),
            &mut outer_cells,
            s.component,
            Some(s.solid.volume_cm3() - grid_volume),
        );
        self.universes.push(Universe {
            id: self.universes.len() as i32,
            cell_indices: outer_cells,
        });
        let outer = self.universes.len() - 1;

        let (lower, _) = lattice.layout(pitch, s.solid.centre_cm());
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

    /// The low-level material index of `m`, resolving it on first use
    /// (equal materials share one entry; nuclides are keyed by nuclide and
    /// thermal law).
    pub(crate) fn material_index(
        &mut self,
        m: &Material,
        t: f64,
        masses: &AtomicMasses,
    ) -> Result<usize, ModelError> {
        if let Some(i) = self.descriptions.iter().position(|d| d == m) {
            return Ok(i);
        }
        let r = m.resolve(masses)?;
        let mut components = Vec::with_capacity(r.nuclides.len());
        for &(isotope, d) in &r.nuclides {
            let key = NuclideKey {
                isotope,
                law: r.moderator.and_then(|md| md.law_for(isotope)),
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
