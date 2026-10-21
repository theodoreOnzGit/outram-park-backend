//! **Lowering a sphere filled with whole particles** (gh #926, tutorial 5 on
//! #943): a lattice in a sphere with whole-particle rejection, and a
//! [`super::DhRegion`] (a lattice or a random packing, delta-tracked).
//!
//! # A lattice in a sphere
//!
//! The grid covers the sphere; each tile's universe is lowered once
//! ([`Lowering::lower_tile_pooled`]); a tile whose contents would poke out of
//! the sphere is replaced by an **empty universe** of the sphere's material,
//! which is also the lattice's `outer`. This is the HTR-10 core's TRISO
//! lattice (`nee_soon::htr10_rmc::core_shell::TrisoBuild::lattice`): its
//! keep rule (`x² + y² + z² <= (R - r)²` at the tile centre, no tolerance,
//! the rule of `pebble_beds::sphere_packing::cubic_array_in_ball`) and its
//! covering grid, expression for expression. Order: the tiles' surfaces and
//! cells, the far plane and the empty cell, the sphere's surface, the fill
//! cell.
//!
//! # A random packing
//!
//! Centres from [`crate::dh_universe::pack_in_ball`] (the packing
//! `DhUniverse::pebble` uses, same arguments, so the same seed gives the
//! same particles); the particle lowered once to a universe of its layers and
//! a matrix cell outside them (as `build_triso_particle` writes its matrix
//! cell, so a point on the particle's surface is never lost); then
//! [`crate::pebble_beds::triso_lattice::create_triso_lattice`] (OpenMC's
//! `create_triso_lattice`) over a tracking grid covering the sphere. The
//! tracking pitch is `(V_particle / fraction)^(1/3)`, about one particle
//! centre per tile: a speed choice that changes no geometry
//! (`tests/model_pebble.rs` checks the lowered pebble gives the material
//! `DhUniverse::pebble`'s own point query gives, at every point sampled).
//!
//! # Volumes
//!
//! The matrix cells of a filled sphere are clipped by it, so none has an
//! exact volume of its own. They are counted in one **pool** per sphere: the
//! sphere's volume minus the whole particles it holds, exact. The census
//! reports the pool as one row and the particles from the geometry.

use petir::mathf::RealMath;
use super::census::Arrangement;
use super::dh_region::{DhSpec, DhTracking, Packing};
use super::lattice::Lattice;
use super::lower::{Lowered, Lowering};
use super::masses::AtomicMasses;
use super::model::ModelError;
use super::shape::{Component, Cube, Shape, Solid, Sphere};
use crate::dh_universe::pack_in_ball;
use crate::geometry::cell::{Cell, CellFill, TrackingMethod};
use crate::geometry::lattice::{Lattice as CsgLattice, RectLattice};
use crate::geometry::position::Position;
use crate::geometry::universe::Universe;
use crate::pebble_beds::sphere_packing::MAX_PF_RSA;
use crate::pebble_beds::triso_lattice::{create_triso_lattice, TrisoLatticeBase};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;

/// The matrix of one sphere filled with whole particles: its cells' volume,
/// counted together, and what the census needs to report the packing.
#[derive(Debug, Clone)]
pub(crate) struct Pool {
    /// The sphere's fill cell (filled with the lattice).
    pub(crate) fill_cell: usize,
    /// The matrix material.
    pub(crate) material: usize,
    /// The matrix's component.
    pub(crate) component: Option<Component>,
    /// Matrix volume per placement of the sphere: the sphere less the whole
    /// particles the lowering placed \[cm³\].
    pub(crate) volume_cm3: f64,
    /// The sphere's volume \[cm³\].
    pub(crate) container_volume_cm3: f64,
    /// The particles' own (outermost) cells and the volume of one particle:
    /// their copies in the geometry are the particle count.
    pub(crate) particle_cells: Vec<(usize, f64)>,
    /// How the particles are arranged.
    pub(crate) arrangement: Arrangement,
    /// The packing fraction asked for (random), or the infinite array's
    /// (a lattice: particle volume over tile volume).
    pub(crate) requested: f64,
    /// Whether the sphere is delta-tracked (a `DhRegion`).
    pub(crate) delta_tracked: bool,
}

/// **Whether `child`, moved by `at`, lies wholly inside `zone`**, with no
/// tolerance: for a sphere, `d² <= (R - r)²` with `d` the centre distance,
/// written as `nee_soon`'s `TrisoBuild::lattice` and `cubic_array_in_ball`
/// write it, so the kept set is theirs bit for bit.
pub(crate) fn whole_in_sphere(zone: &Sphere, child: &Solid, at: [f64; 3]) -> bool {
    let c = zone.centre_cm();
    let rz = zone.radius_cm();
    match child {
        Solid::Sphere(p) => {
            let pc = p.centre_cm();
            let d: [f64; 3] = std::array::from_fn(|i| (at[i] + pc[i]) - c[i]);
            let keep = rz - p.radius_cm();
            keep >= 0.0 && d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= keep * keep
        }
        Solid::Cuboid(b) => {
            let (l, u) = b.bounds();
            let d2: f64 = (0..3)
                .map(|k| {
                    let a = (at[k] + l[k] - c[k]).abs();
                    let b = (at[k] + u[k] - c[k]).abs();
                    a.max(b).r_powi(2)
                })
                .sum();
            d2 <= rz * rz
        }
        Solid::HexPrism(h) => h
            .vertices()
            .iter()
            .all(|v| (0..3).map(|k| (at[k] + v[k] - c[k]).r_powi(2)).sum::<f64>() <= rz * rz),
        Solid::Cylinder(cy) => {
            // A rod has no ends and so is never whole in a sphere; a finite
            // cylinder's farthest point is on the rim of its farther end.
            let (lo, hi) = cy.z_range();
            if !(lo.is_finite() && hi.is_finite()) {
                return false;
            }
            let cc = cy.centre_cm();
            let rim = ((at[0] + cc[0]) - c[0]).r_hypot((at[1] + cc[1]) - c[1]) + cy.radius_cm();
            let dz = (at[2] + lo - c[2]).abs().max((at[2] + hi - c[2]).abs());
            rim.r_hypot(dz) <= rz
        }
        // A hopper is never a packed particle; its bounding box's corners
        // decide (which can only refuse).
        Solid::Hopper(h) => {
            let (l, u) = h.bounds();
            (0..8).all(|k| {
                (0..3)
                    .map(|d| (at[d] + if (k >> d) & 1 == 0 { l[d] } else { u[d] } - c[d]).r_powi(2))
                    .sum::<f64>()
                    <= rz * rz
            })
        }
    }
}

/// **The lattice a lattice-packed [`super::DhRegion`] lowers to**: a cube of
/// the region's matrix around the particle, repeated by
/// [`Lattice::covering`]. `None` for a random packing. Shared by the checks
/// and the lowering, so both see the same lattice.
pub(crate) fn dh_lattice(s: &Shape, spec: &DhSpec) -> Result<Option<Lattice>, ModelError> {
    let Some((particle, packing)) = &spec.particles else {
        return Err(ModelError::BadValue(
            "a DhRegion needs its particles: .with_particles(particle, packing)".into(),
        ));
    };
    let Solid::Sphere(zone) = s.solid else {
        return Err(ModelError::BadValue("a DhRegion is a sphere".into()));
    };
    let r = particle.packed_radius_cm()?;
    let (cube, offset) = match *packing {
        Packing::Random { .. } => return Ok(None),
        Packing::Lattice { pitch, offset } => {
            let p = pitch.get::<centimeter>();
            if !(p.is_finite() && p >= 2.0 * r) {
                return Err(ModelError::BadValue(format!(
                    "a lattice pitch of {p} cm is smaller than the particle's diameter {} cm: \
                     neighbours would overlap",
                    2.0 * r
                )));
            }
            (Cube::side(pitch), offset)
        }
        Packing::LatticeOfCount { particles, offset } => {
            if particles == 0 {
                return Err(ModelError::BadValue(
                    "a lattice of zero particles holds nothing; use a plain sphere".into(),
                ));
            }
            (
                Cube::for_whole_particles_in_sphere(particle, zone, particles, offset)?,
                offset,
            )
        }
    };
    let mut tile = cube
        .filled_with(s.material.clone())
        .containing((**particle).clone());
    tile.component = s.component;
    // The zone's own temperature, so the tile resolves to the zone's (it
    // shares the zone's component; gh #945).
    tile.temperature = s.temperature;
    Ok(Some(Lattice::covering(tile, offset)))
}

/// **The checks a [`super::DhRegion`] must pass** before lowering (tracking
/// set, particles given and fitting, a reachable packing fraction). Returns
/// the lattice of a lattice packing, for the caller to check its tile.
pub(crate) fn check_dh(s: &Shape, spec: &DhSpec) -> Result<Option<Lattice>, ModelError> {
    if !s.children.is_empty() || s.lattice.is_some() {
        return Err(ModelError::BadValue(
            "a DhRegion holds only its packed particles; put other shapes outside it".into(),
        ));
    }
    if spec.tracking.is_none() {
        return Err(ModelError::BadValue(
            "a DhRegion needs its tracking: call .delta_tracking() (#943: there is no default)"
                .into(),
        ));
    }
    let lattice = dh_lattice(s, spec)?;
    let (Solid::Sphere(zone), Some((particle, packing))) = (s.solid, &spec.particles) else {
        return Err(ModelError::BadValue(
            "a DhRegion is a sphere with particles".into(),
        ));
    };
    let r = particle.packed_radius_cm()?;
    if !(r < zone.radius_cm()) {
        return Err(ModelError::BadValue(format!(
            "the particle (radius {r} cm) is larger than the region it is packed in (radius {} cm)",
            zone.radius_cm()
        )));
    }
    if let Packing::Random { fraction, .. } = packing {
        let pf = fraction.get::<ratio>();
        if !(pf > 0.0 && pf <= MAX_PF_RSA) {
            return Err(ModelError::BadValue(format!(
                "a packing fraction of {pf} is unreachable by random sequential addition: it \
                 must be above 0 and at most {MAX_PF_RSA} (RSA jams near 38 %, MAX_PF_RSA)"
            )));
        }
        let expected = pf * (zone.radius_cm() / r).r_powi(3);
        if expected < 1.0 {
            return Err(ModelError::BadValue(format!(
                "a packing fraction of {pf} puts {expected:.2} particles in this region: fewer \
                 than one"
            )));
        }
    }
    if let Some(why) = lattice.as_ref().and_then(Lattice::problem) {
        return Err(ModelError::BadValue(why));
    }
    Ok(lattice)
}

impl Lowering {
    /// Reserve a pool; its fields are filled when the sphere is lowered.
    fn new_pool(&mut self, s: &Shape) -> usize {
        self.pools.push(Pool {
            fill_cell: usize::MAX,
            material: usize::MAX,
            component: s.component,
            volume_cm3: f64::NAN,
            container_volume_cm3: s.solid.volume_cm3(),
            particle_cells: Vec::new(),
            arrangement: Arrangement::Random {
                seed: 0,
                tracking_pitch_cm: f64::NAN,
                grid: [0; 3],
            },
            requested: f64::NAN,
            delta_tracked: false,
        });
        self.pools.len() - 1
    }

    /// The sphere's surfaces and its fill cell (filled with lattice `li`).
    fn sphere_fill_cell(
        &mut self,
        s: &Shape,
        li: usize,
        cells_of: &mut Vec<usize>,
    ) -> (Lowered, usize) {
        let (inside, outside, surfaces) = self.solid_surfaces(&s.solid);
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::fill(id, inside, CellFill::Lattice(li), Position::ZERO),
            cells_of,
            s.component,
            None,
        );
        (Lowered { outside, surfaces }, self.cells.len() - 1)
    }

    /// **A lattice in a sphere, with whole-particle rejection** (see the
    /// module docs).
    pub(crate) fn lower_lattice_in_sphere(
        &mut self,
        s: &Shape,
        zone: Sphere,
        lattice: &Lattice,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        let pitch = lattice
            .pitch_cm()
            .ok_or_else(|| ModelError::BadValue("a lattice tile must be a box".into()))?;
        let centre = zone.centre_cm();
        let (n, lower) = match lattice.covering_offset() {
            Some(off) => Lattice::covering_grid(zone.radius_cm(), pitch, off, centre),
            None => (lattice.dimensions(), lattice.layout(pitch, centre).0),
        };
        let pool = self.new_pool(s);
        let was = (self.in_tile, self.tile_has_heavy_metal);
        self.in_tile = true;
        self.tile_has_heavy_metal = false;
        let tiles: Vec<(usize, Vec<(usize, f64)>)> = lattice
            .tiles()
            .iter()
            .map(|tile| self.lower_tile_pooled(tile, t, masses, Some(pool)))
            .collect::<Result<_, _>>()?;
        let tiles_hold_heavy_metal = self.tile_has_heavy_metal;
        self.in_tile = was.0;
        self.tile_has_heavy_metal = was.1 || tiles_hold_heavy_metal;

        // The empty universe: the sphere's material everywhere.
        let m = self.material_index(&s.material, t, masses)?;
        let everywhere = self.everywhere();
        let mut empty_cells = Vec::new();
        let id = self.cells.len() as i32 + 1;
        self.push_cell(
            Cell::material(id, everywhere, m, t),
            &mut empty_cells,
            s.component,
            None,
        );
        if let Some(p) = self.cell_pools.last_mut() {
            *p = Some(pool);
        }
        self.universes.push(Universe {
            id: self.universes.len() as i32,
            cell_indices: empty_cells,
        });
        let empty = self.universes.len() - 1;

        // Whole-particle rejection, per tile (TrisoBuild::lattice's loop).
        let mut kept = vec![0usize; tiles.len()];
        let mut universes = Vec::with_capacity(n[0] * n[1] * n[2]);
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let flat = n[0] * n[1] * k + n[0] * j + i;
                    let which = match lattice.covering_offset() {
                        Some(_) => 0,
                        None => lattice.pattern()[flat],
                    };
                    let c = |lo: f64, idx: usize, p: f64| lo + (idx as f64 + 0.5) * p;
                    let at = [
                        c(lower[0], i, pitch[0]),
                        c(lower[1], j, pitch[1]),
                        c(lower[2], k, pitch[2]),
                    ];
                    let whole = lattice.tiles()[which]
                        .children
                        .iter()
                        .all(|ch| whole_in_sphere(&zone, &ch.solid, at));
                    if whole {
                        kept[which] += 1;
                        universes.push(tiles[which].0);
                    } else {
                        universes.push(empty);
                    }
                }
            }
        }
        self.lattices.push(CsgLattice::Rect(RectLattice {
            id: self.lattices.len() as i32 + 1,
            n,
            lower_left: Position::new(lower[0], lower[1], lower[2]),
            pitch,
            universes,
            outer: Some(empty),
        }));
        let li = self.lattices.len() - 1;
        if tiles_hold_heavy_metal {
            self.note_heavy_metal_bounds(&s.solid);
        }
        let (lowered, fill_cell) = self.sphere_fill_cell(s, li, cells_of);

        let placed: f64 = tiles
            .iter()
            .zip(&kept)
            .map(|((_, ch), &k)| k as f64 * ch.iter().map(|c| c.1).sum::<f64>())
            .sum();
        let first = &lattice.tiles()[0];
        let p = &mut self.pools[pool];
        p.fill_cell = fill_cell;
        p.material = m;
        p.volume_cm3 = s.solid.volume_cm3() - placed;
        p.particle_cells = tiles
            .iter()
            .zip(&kept)
            .filter(|(_, &k)| k > 0)
            .flat_map(|((_, ch), _)| ch.iter().copied())
            .collect();
        p.arrangement = Arrangement::RegularLattice {
            pitch_cm: pitch,
            grid: n,
            offset: lattice.covering_offset(),
        };
        p.requested = first
            .children
            .iter()
            .map(|c| c.solid.volume_cm3())
            .sum::<f64>()
            / first.solid.volume_cm3();
        Ok(lowered)
    }

    /// **A [`super::DhRegion`]**: its packing, then its tracking on the fill
    /// cell and a majorant region over every material it presents.
    pub(crate) fn lower_dh(
        &mut self,
        s: &Shape,
        spec: &DhSpec,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        let Solid::Sphere(zone) = s.solid else {
            return Err(ModelError::BadValue("a DhRegion is a sphere".into()));
        };
        let first_cell = self.cells.len();
        let lowered = match (dh_lattice(s, spec)?, &spec.particles) {
            (Some(lattice), _) => {
                self.lower_lattice_in_sphere(s, zone, &lattice, t, masses, cells_of)?
            }
            (None, Some((particle, Packing::Random { fraction, seed }))) => self.lower_random(
                s,
                zone,
                particle,
                fraction.get::<ratio>(),
                *seed,
                t,
                masses,
                cells_of,
            )?,
            (None, _) => {
                return Err(ModelError::BadValue(
                    "a DhRegion needs its particles: .with_particles(particle, packing)".into(),
                ))
            }
        };
        match spec.tracking {
            None => Err(ModelError::BadValue(
                "a DhRegion needs its tracking: call .delta_tracking() (#943: there is no default)"
                    .into(),
            )),
            Some(DhTracking::Delta) => {
                let mut mats: Vec<usize> = self.cells[first_cell..]
                    .iter()
                    .filter_map(|c| match c.fill {
                        CellFill::Material(m) => Some(m),
                        _ => None,
                    })
                    .collect();
                mats.sort_unstable();
                mats.dedup();
                self.delta_regions.push(mats);
                let majorant = self.delta_regions.len() - 1;
                let fill = self.cells.len() - 1;
                self.cells[fill].tracking = Some(TrackingMethod::Delta { majorant });
                if let Some(p) = self.pools.last_mut() {
                    p.delta_tracked = true;
                }
                Ok(lowered)
            }
        }
    }

    /// **A random packing in a sphere** (see the module docs).
    #[allow(clippy::too_many_arguments)]
    fn lower_random(
        &mut self,
        s: &Shape,
        zone: Sphere,
        particle: &Shape,
        pf: f64,
        seed: u64,
        t: f64,
        masses: &AtomicMasses,
        cells_of: &mut Vec<usize>,
    ) -> Result<Lowered, ModelError> {
        let r = particle.packed_radius_cm()?;
        let big_r = zone.radius_cm();
        let packed = pack_in_ball(r, big_r, pf, seed)
            .map_err(|e| ModelError::BadValue(format!("the random packing failed: {e}")))?;
        // pack_in_ball aims for 0.2 % of the fraction; with few particles one
        // particle is a bigger step than that.
        let one = (r / big_r).r_powi(3);
        if (packed.realised - pf).abs() > (2.0e-3 * pf).max(one) {
            return Err(ModelError::BadValue(format!(
                "random packing reached {:.6} where {pf:.6} was asked (more than 0.2 % or one \
                 particle away); the fraction is too close to what RSA can place here",
                packed.realised
            )));
        }
        let c = zone.centre_cm();
        let centres: Vec<Position> = packed
            .packing
            .spheres()
            .iter()
            .map(|p| Position::new(p.center.x + c[0], p.center.y + c[1], p.center.z + c[2]))
            .collect();

        let pool = self.new_pool(s);
        // The particle universe: its layers, and the matrix outside them.
        let carrier = Shape {
            solid: s.solid,
            material: s.material.clone(),
            component: s.component,
            children: vec![particle.clone()],
            lattice: None,
            boundary: None,
            dh: None,
            // The zone's own temperature, so the carrier resolves to the
            // zone's (it shares the zone's component; gh #945).
            temperature: s.temperature,
        };
        let was = (self.in_tile, self.tile_has_heavy_metal);
        self.in_tile = true;
        self.tile_has_heavy_metal = false;
        let (particle_universe, particle_cells) =
            self.lower_tile_pooled(&carrier, t, masses, Some(pool))?;
        let holds_heavy_metal = self.tile_has_heavy_metal;
        self.in_tile = was.0;
        self.tile_has_heavy_metal = was.1 || holds_heavy_metal;

        // The tracking lattice: about one particle centre per tile.
        let v_particle = particle.solid.volume_cm3();
        let p = (v_particle / pf).r_cbrt();
        let pitch = [p; 3];
        let (n, lower) = Lattice::covering_grid(big_r, pitch, [0.5; 3], c);
        let m = self.material_index(&s.material, t, masses)?;
        let base = TrisoLatticeBase {
            surface: self.surfaces.len(),
            cell: self.cells.len(),
            universe: self.universes.len(),
        };
        let built = create_triso_lattice(
            &centres,
            r,
            particle_universe,
            Position::new(lower[0], lower[1], lower[2]),
            pitch,
            n,
            m,
            t,
            base,
            self.lattices.len() as i32 + 1,
        );
        if built.partly_outside > 0 {
            return Err(ModelError::BadValue(format!(
                "{} particles reach outside the tracking lattice; this is a defect (the grid \
                 covers the sphere)",
                built.partly_outside
            )));
        }
        self.surfaces.extend(built.surfaces);
        for (j, cell) in built.cells.into_iter().enumerate() {
            let background = built.background_cells.contains(&(base.cell + j));
            self.cells.push(cell);
            self.cell_components
                .push(if background { s.component } else { None });
            self.cell_volumes.push(None);
            self.cell_pools.push(background.then_some(pool));
        }
        self.universes.extend(built.universes);
        self.lattices.push(CsgLattice::Rect(built.lattice));
        let li = self.lattices.len() - 1;
        if holds_heavy_metal {
            self.note_heavy_metal_bounds(&s.solid);
        }
        let (lowered, fill_cell) = self.sphere_fill_cell(s, li, cells_of);

        let pl = &mut self.pools[pool];
        pl.fill_cell = fill_cell;
        pl.material = m;
        pl.volume_cm3 = s.solid.volume_cm3() - centres.len() as f64 * v_particle;
        pl.particle_cells = particle_cells;
        pl.arrangement = Arrangement::Random {
            seed,
            tracking_pitch_cm: p,
            grid: n,
        };
        pl.requested = pf;
        Ok(lowered)
    }
}
