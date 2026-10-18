//! **What the assembled geometry holds: copies and volumes** (gh #926).
//!
//! Counted from the **assembled** CSG, never from the numbers the model was
//! built with: the HTR-10 history had a TRISO lattice holding 8240
//! particles while the model reported 8340 (gh:#316, +353 pcm), because the
//! count came from a constant. Here the copies of each cell are counted by a
//! walk of the geometry's universe and lattice graph from the root, and each
//! copy's volume is exact (a shape's solid minus the solids it contains,
//! from the lowering).
//!
//! **A copy is a distinct place in space.** ~~The copies of each cell are the
//! distributed-cell instance count (`DistribcellOffsets`).~~ **CHANGED
//! 2026-10-18 (#926, random packing):** a randomly packed particle that
//! straddles a tracking-lattice face is drawn in both tiles (OpenMC's
//! `create_triso_lattice`), so it is reached by two paths, and a path count
//! (distribcell's) would count it twice. The walk keeps each universe's
//! **placements** (its origin in the root frame, to 1e-9 cm) and counts
//! distinct ones. Where no universe is reached twice at one place (every
//! model before random packing), this is the distribcell count, and
//! `tests/model_repeated_structures.rs` still pins it. A lattice's `outer`
//! is counted once per placement of the lattice, as before.
//!
//! **Pooled volumes.** The matrix cells of a sphere filled with whole
//! particles (a lattice in a sphere, a `DhRegion`) are clipped by the
//! sphere, so none has an exact volume of its own. Their rows say
//! [`RowKind::Pooled`] with no volume, and one [`RowKind::Pool`] row per
//! sphere carries the matrix's exact volume: the sphere less the whole
//! particles in it. [`Census::packings`] reports each such sphere: the
//! particles counted from the geometry and the packing fraction they
//! realise, beside the one asked for.

use std::collections::HashSet;

use uom::si::f64::{Ratio, Volume};
use uom::si::ratio::ratio;
use uom::si::volume::cubic_centimeter;

use super::model::{AssembledModel, ModelError};
use super::shape::Component;
use crate::geometry::cell::CellFill;
use crate::geometry::geometry::Geometry;
use crate::geometry::lattice::Lattice as CsgLattice;

/// How a census row's volume is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// An ordinary cell: `volume_each` is exact.
    Cell,
    /// A matrix cell clipped by the sphere it fills: no volume of its own
    /// (`volume_each` is zero); the [`RowKind::Pool`] row of the sphere's
    /// fill cell `pool_cell` carries it.
    Pooled {
        /// The fill cell whose pool row carries this cell's volume.
        pool_cell: usize,
    },
    /// The matrix of a sphere filled with whole particles, all its cells
    /// together: `volume_each` is the sphere less its particles, exact;
    /// `copies` the placements of the sphere.
    Pool,
}

/// One material cell of the assembled geometry (or one pool; see
/// [`RowKind`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CensusRow {
    /// The cell's index in the assembled geometry (for a pool row, the
    /// sphere's fill cell).
    pub cell: usize,
    /// Its material's name.
    pub material: String,
    /// Its component, if its shape had one.
    pub component: Option<Component>,
    /// That component's label.
    pub component_label: Option<String>,
    /// How many copies of the cell the geometry places: distinct places in
    /// space, through every lattice tile and nested universe.
    pub copies: usize,
    /// The exact volume of one copy (zero for a pooled cell).
    pub volume_each: Volume,
    /// How the volume is counted.
    pub kind: RowKind,
}

impl CensusRow {
    /// `copies × volume_each`.
    pub fn volume(&self) -> Volume {
        self.volume_each * self.copies as f64
    }
}

/// How the particles of a packed sphere are arranged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arrangement {
    /// A regular lattice of tiles with whole-particle rejection.
    RegularLattice {
        /// The pitch \[cm\].
        pitch_cm: [f64; 3],
        /// The grid laid over the sphere.
        grid: [usize; 3],
        /// The offset in pitches, for a covering lattice.
        offset: Option<[f64; 3]>,
    },
    /// Random sequential addition (`pack_in_ball`), placed in a tracking
    /// lattice (`create_triso_lattice`).
    Random {
        /// The packing seed.
        seed: u64,
        /// The tracking lattice's pitch \[cm\] (a speed choice only).
        tracking_pitch_cm: f64,
        /// The tracking grid.
        grid: [usize; 3],
    },
}

/// **A sphere filled with whole particles**: what the geometry holds,
/// against what was asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct PackingRow {
    /// The sphere's fill cell.
    pub cell: usize,
    /// The matrix's component.
    pub component: Option<Component>,
    /// That component's label.
    pub component_label: Option<String>,
    /// The arrangement.
    pub arrangement: Arrangement,
    /// Whether the sphere is delta-tracked (a `DhRegion`).
    pub delta_tracked: bool,
    /// Placements of the sphere in the geometry.
    pub placements: usize,
    /// **Particles counted from the assembled geometry** (every placement).
    pub particles: usize,
    /// Their volume, together.
    pub particle_volume: Volume,
    /// The spheres' volume, together.
    pub region_volume: Volume,
    /// The packing fraction asked for: the random packing's fraction, or a
    /// lattice's infinite-array fraction (particle over tile volume).
    pub requested: Ratio,
    /// **The packing fraction the geometry realises**: particle volume over
    /// sphere volume. It differs from `requested` by whole-particle
    /// rejection (a lattice) or by the packer's tolerance (random).
    pub realised: Ratio,
}

/// **Every material cell, how many copies the geometry places, and their
/// volumes**, plus every sphere filled with particles. Made by
/// [`AssembledModel::census`].
#[derive(Debug, Clone, PartialEq)]
pub struct Census {
    /// One row per material cell, in cell order, then one
    /// [`RowKind::Pool`] row per packed sphere.
    pub rows: Vec<CensusRow>,
    /// One row per sphere filled with whole particles.
    pub packings: Vec<PackingRow>,
}

impl Census {
    /// **How many shapes of `component` the geometry places**: the copies of
    /// every cell in it, summed (pool rows excluded). For a component holding
    /// one shape per particle (a TRISO kernel), this is the particle count.
    pub fn copies(&self, component: Component) -> usize {
        self.rows
            .iter()
            .filter(|r| r.component == Some(component) && r.kind != RowKind::Pool)
            .map(|r| r.copies)
            .sum()
    }

    /// The total volume of `component`'s cells.
    pub fn volume(&self, component: Component) -> Volume {
        self.rows
            .iter()
            .filter(|r| r.component == Some(component))
            .map(CensusRow::volume)
            .fold(Volume::new::<cubic_centimeter>(0.0), |a, b| a + b)
    }

    /// The volume of every material cell together (void excluded).
    pub fn total_volume(&self) -> Volume {
        self.rows
            .iter()
            .map(CensusRow::volume)
            .fold(Volume::new::<cubic_centimeter>(0.0), |a, b| a + b)
    }

    /// `component`'s share of [`Self::total_volume`].
    pub fn volume_fraction(&self, component: Component) -> Ratio {
        self.volume(component) / self.total_volume()
    }
}

impl std::fmt::Display for Census {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{:>5}  {:<28} {:<14} {:>8} {:>14} {:>14} {:>9}",
            "cell", "material", "component", "copies", "each [cm3]", "total [cm3]", "share"
        )?;
        let total = self.total_volume().get::<cubic_centimeter>();
        // Pooled cells (a random packing has thousands) are summarised on
        // their pool's row, not listed one by one.
        let pooled = |cell: usize| {
            self.rows
                .iter()
                .filter(|r| r.kind == RowKind::Pooled { pool_cell: cell })
                .count()
        };
        for r in &self.rows {
            let v = r.volume().get::<cubic_centimeter>();
            if let RowKind::Pooled { .. } = r.kind {
                continue;
            }
            let each = format!("{:.6e}", r.volume_each.get::<cubic_centimeter>());
            let name = match r.kind {
                RowKind::Pool => format!("{} ({} cells)", r.material, pooled(r.cell)),
                _ => r.material.clone(),
            };
            writeln!(
                f,
                "{:>5}  {:<28} {:<14} {:>8} {:>14} {:>14.6e} {:>8.5}%",
                r.cell,
                name,
                r.component_label.as_deref().unwrap_or("-"),
                r.copies,
                each,
                v,
                100.0 * v / total
            )?;
        }
        write!(f, "total volume {total:.6e} cm3")?;
        for p in &self.packings {
            let how = match p.arrangement {
                Arrangement::RegularLattice { pitch_cm, grid, .. } => format!(
                    "regular lattice, pitch {:.7} cm, {}x{}x{} grid",
                    pitch_cm[0], grid[0], grid[1], grid[2]
                ),
                Arrangement::Random { seed, .. } => format!("random (RSA), seed {seed}"),
            };
            write!(
                f,
                "\npacked sphere (cell {}, {how}{}): {} particles; packing fraction {:.6} % \
                 (asked {:.6} %)",
                p.cell,
                if p.delta_tracked {
                    ", delta tracking"
                } else {
                    ""
                },
                p.particles,
                100.0 * p.realised.get::<ratio>(),
                100.0 * p.requested.get::<ratio>()
            )?;
        }
        Ok(())
    }
}

/// Placements are compared on this grid \[cm\]: two placements of a universe
/// closer than this are one copy.
const SAME_PLACE_CM: f64 = 1.0e-9;

/// **Distinct placements of every universe** of `g` (see the module docs).
fn universe_placements(g: &Geometry) -> Result<Vec<usize>, ModelError> {
    type Key = (i64, i64, i64, usize);
    let q = |o: [f64; 3]| -> [i64; 3] { o.map(|x| (x / SAME_PLACE_CM).round() as i64) };
    let mut seen: Vec<HashSet<Key>> = vec![HashSet::new(); g.universes.len()];
    let mut stack = vec![(g.root_universe, [0.0_f64; 3], 0_usize)];
    while let Some((u, origin, tag)) = stack.pop() {
        let set = seen
            .get(u)
            .ok_or_else(|| ModelError::BadValue(format!("universe {u} out of range (a defect)")))?;
        let k = q(origin);
        let mut known = false;
        'near: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if set.contains(&(k[0] + dx, k[1] + dy, k[2] + dz, tag)) {
                        known = true;
                        break 'near;
                    }
                }
            }
        }
        if known {
            continue;
        }
        seen[u].insert((k[0], k[1], k[2], tag));
        for &ci in &g.universes[u].cell_indices {
            let c = &g.cells[ci];
            let o = [
                origin[0] + c.translation.x,
                origin[1] + c.translation.y,
                origin[2] + c.translation.z,
            ];
            match c.fill {
                CellFill::Universe(v) => stack.push((v, o, 0)),
                CellFill::Lattice(li) => {
                    match &g.lattices[li] {
                        CsgLattice::Rect(l) => {
                            for iz in 0..l.n[2] {
                                for iy in 0..l.n[1] {
                                    for ix in 0..l.n[0] {
                                        let flat = l.n[0] * l.n[1] * iz + l.n[0] * iy + ix;
                                        let t = l.tile_center([ix as i32, iy as i32, iz as i32]);
                                        stack.push((
                                            l.universes[flat],
                                            [o[0] + t.x, o[1] + t.y, o[2] + t.z],
                                            0,
                                        ));
                                    }
                                }
                            }
                            if let Some(out) = l.outer {
                                stack.push((out, o, 1 + li));
                            }
                        }
                        CsgLattice::Hex(_) => return Err(ModelError::BadValue(
                            "the census does not count hex lattices yet (the beginner API makes \
                             none)"
                                .into(),
                        )),
                    }
                }
                CellFill::Material(_) | CellFill::Void => {}
            }
        }
    }
    Ok(seen.iter().map(HashSet::len).collect())
}

impl AssembledModel {
    /// **Count and measure what the assembled geometry holds** (see the
    /// module docs for why it is counted from the geometry, and what a copy
    /// is).
    ///
    /// # Errors
    /// [`ModelError::BadValue`] if the geometry's universe graph cannot be
    /// walked (an index out of range; an assembled model never has one, so
    /// this is a defect if seen), or holds a hex lattice (the beginner API
    /// makes none).
    pub fn census(&self) -> Result<Census, ModelError> {
        let placed = universe_placements(&self.geometry)?;
        let mut universe_of = vec![usize::MAX; self.geometry.cells.len()];
        for (u, uni) in self.geometry.universes.iter().enumerate() {
            for &c in &uni.cell_indices {
                universe_of[c] = u;
            }
        }
        let copies = |c: usize| placed.get(universe_of[c]).copied().unwrap_or(0);
        let label =
            |c: Option<Component>| c.and_then(|c| self.component_labels.get(c.index).cloned());
        let mut rows = Vec::new();
        for (i, cell) in self.geometry.cells.iter().enumerate() {
            let CellFill::Material(m) = cell.fill else {
                continue;
            };
            let (kind, each) = match self.cell_pools.get(i).copied().flatten() {
                Some(p) => (
                    RowKind::Pooled {
                        pool_cell: self.pools[p].fill_cell,
                    },
                    0.0,
                ),
                None => (RowKind::Cell, self.cell_volumes_cm3[i].unwrap_or(f64::NAN)),
            };
            let component = self.cell_components[i];
            rows.push(CensusRow {
                cell: i,
                material: self.resolved[m].name.clone(),
                component,
                component_label: label(component),
                copies: copies(i),
                volume_each: Volume::new::<cubic_centimeter>(each),
                kind,
            });
        }
        let mut packings = Vec::new();
        for p in &self.pools {
            let placements = copies(p.fill_cell);
            rows.push(CensusRow {
                cell: p.fill_cell,
                material: self.resolved[p.material].name.clone(),
                component: p.component,
                component_label: label(p.component),
                copies: placements,
                volume_each: Volume::new::<cubic_centimeter>(p.volume_cm3),
                kind: RowKind::Pool,
            });
            let particles: usize = p.particle_cells.iter().map(|&(c, _)| copies(c)).sum();
            let particle_v: f64 = p
                .particle_cells
                .iter()
                .map(|&(c, v)| copies(c) as f64 * v)
                .sum();
            let region_v = placements as f64 * p.container_volume_cm3;
            packings.push(PackingRow {
                cell: p.fill_cell,
                component: p.component,
                component_label: label(p.component),
                arrangement: p.arrangement,
                delta_tracked: p.delta_tracked,
                placements,
                particles,
                particle_volume: Volume::new::<cubic_centimeter>(particle_v),
                region_volume: Volume::new::<cubic_centimeter>(region_v),
                requested: Ratio::new::<ratio>(p.requested),
                realised: Ratio::new::<ratio>(particle_v / region_v),
            });
        }
        Ok(Census { rows, packings })
    }
}
