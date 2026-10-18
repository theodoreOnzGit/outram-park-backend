//! **Repeated structures: a lattice of tiles** (gh #926, epic #925).
//!
//! A tile is an ordinary [`Shape`]: a [`Cuboid`](super::Cuboid) (its sides
//! are the lattice pitch) filled with a background material, containing
//! whatever repeats (a TRISO particle, a pin). A [`Lattice`] arranges tiles
//! on a rectangular grid; a container holds it with
//! [`Shape::containing_lattice`]. The beginner never meets a universe: each
//! distinct tile is lowered once to a universe, and the grid to the CSG
//! [`RectLattice`](crate::geometry::lattice::RectLattice) every lattice
//! model in this crate uses.
//!
//! **Row order is said out loud** (#926, the LCT-008 trap): OpenMC's
//! `universes` array runs **top row first** (`openmc/lattice.py:382-388`),
//! outram's CSG lattice starts at the **lower-left** (`rect.rs:45-47`), and
//! the LCT-008 port needed a `reverse_rows` flag for it
//! (`examples/common/lct008_model.rs:440`). Here the reader picks the order
//! by the constructor's name: [`Lattice::from_rows_top_down`] (as a picture
//! of the lattice is drawn, and as OpenMC's input reads) or
//! [`Lattice::from_natural_order`] (lower-left first, x fastest).
//!
//! Tracking is surface tracking (the default for every region, decided on
//! #938/#943). The optional per-lattice override (`.tracking(..)`, e.g.
//! delta tracking over a dense lattice) needs majorants the beginner run
//! does not carry yet, so it is not offered; see #926.

use super::shape::{Shape, Solid};

/// **A rectangular lattice of tiles.** Build it with one of the named
/// constructors; [`super::Model::assemble`] checks it (every tile a box of
/// the same sides centred on its own origin, every pattern entry naming a
/// tile, the pattern the size of the grid) and refuses it otherwise.
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Length, length::centimeter};
/// let cm = Length::new::<centimeter>;
/// let graphite = Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]);
/// let fuel = Material::from_atom_densities([(U235, atoms_per_barn_cm(0.04))]);
/// let ball = Sphere::radius(cm(0.4)).filled_with(fuel);
/// let tile = Cube::side(cm(1.0)).filled_with(graphite.clone()).containing(ball);
/// let lattice = Lattice::copies_of(tile, [3, 3, 3]);
/// assert_eq!(lattice.dimensions(), [3, 3, 3]);
/// let block = Cube::side(cm(3.0)).filled_with(graphite).containing_lattice(lattice);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Lattice {
    /// The distinct tiles.
    tiles: Vec<Shape>,
    /// Tiles per axis.
    n: [usize; 3],
    /// Per grid position, in natural order (`nx*ny*iz + nx*iy + ix`), the
    /// tile index.
    pattern: Vec<usize>,
    /// `Some(offset)` for a [`Lattice::covering`] lattice: the grid is
    /// sized when assembled, to cover its container.
    covering: Option<[f64; 3]>,
}

impl Lattice {
    /// **`n[0] x n[1] x n[2]` copies of one tile.** The pitch is the tile's
    /// sides. A unit cell repeated is a regular array: tutorial 4's TRISO
    /// cell, stacked.
    pub fn copies_of(tile: Shape, n: [usize; 3]) -> Self {
        let count = n[0] * n[1] * n[2];
        Self {
            tiles: vec![tile],
            n,
            pattern: vec![0; count],
            covering: None,
        }
    }

    /// **As many copies of one tile as cover the sphere that holds it**
    /// (#926, the pebble's fuel zone), with **whole-particle rejection**: a
    /// tile whose contents would poke out of the sphere is left out, and the
    /// sphere's material fills its place. Nothing is ever cut.
    ///
    /// `offset` places the grid: one tile centre sits at `offset[i]` pitches
    /// from the sphere's centre along axis `i` (`[0.0; 3]` puts a tile at the
    /// centre, `[0.5; 3]` puts eight tile corners there). The count of whole
    /// tiles moves in symmetry shells as the pitch changes, and a generic
    /// offset breaks the symmetry; see
    /// [`crate::pebble_beds::sphere_packing::cubic_array_in_ball`], whose
    /// convention this is, and [`super::Cube::for_whole_particles_in_sphere`]
    /// to choose the pitch for a particle count.
    ///
    /// The grid size is chosen when the model is assembled, by the covering
    /// rule `nee_soon::htr10_rmc::core_shell::TrisoBuild` uses for HTR-10's
    /// fuel zone (the grid must cover the sphere; a grid that stops short
    /// left a shell inside the cell but outside the lattice and gave
    /// negative distances to boundary there, gh:#316 history), so
    /// [`Self::dimensions`] is `[0, 0, 0]` until then; the census and the
    /// assembled geometry hold the built grid. Only a [`super::Sphere`] can
    /// hold one; a box refuses it.
    ///
    /// ```
    /// use outram_mc_libs::model::prelude::*;
    /// use uom::si::{f64::Length, length::centimeter};
    /// let cm = Length::new::<centimeter>;
    /// let graphite = Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]);
    /// let fuel = Material::from_atom_densities([(U235, atoms_per_barn_cm(0.04))]);
    /// let tile = Cube::side(cm(0.5))
    ///     .filled_with(graphite.clone())
    ///     .containing(Sphere::radius(cm(0.1)).filled_with(fuel));
    /// let zone = Sphere::radius(cm(2.0))
    ///     .filled_with(graphite)
    ///     .containing_lattice(Lattice::covering(tile, [0.13, 0.37, 0.71]));
    /// assert_eq!(zone.contained_lattice().unwrap().covering_offset(), Some([0.13, 0.37, 0.71]));
    /// ```
    pub fn covering(tile: Shape, offset: [f64; 3]) -> Self {
        Self {
            tiles: vec![tile],
            n: [0; 3],
            pattern: Vec::new(),
            covering: Some(offset),
        }
    }

    /// The offset of a [`Self::covering`] lattice, in pitches; `None` for a
    /// lattice of given size.
    pub fn covering_offset(&self) -> Option<[f64; 3]> {
        self.covering
    }

    /// **Tiles in natural order**: `pattern[nx*ny*iz + nx*iy + ix]` is the
    /// index into `tiles` of the tile at `(ix, iy, iz)`, counted from the
    /// **lower-left** (smallest x, y, z), x fastest. This is the CSG
    /// lattice's own order (`geometry::lattice::RectLattice::universes`).
    pub fn from_natural_order(tiles: Vec<Shape>, n: [usize; 3], pattern: Vec<usize>) -> Self {
        Self {
            tiles,
            n,
            pattern,
            covering: None,
        }
    }

    /// **One layer of tiles, written as you would draw it: the first row is
    /// the TOP row** (largest y), each row left to right (increasing x).
    /// OpenMC's `RectLattice.universes` reads the same way. `rows[r][c]` is
    /// the index into `tiles` of the tile in row `r` from the top, column
    /// `c`. All rows must be the same length.
    pub fn from_rows_top_down(tiles: Vec<Shape>, rows: Vec<Vec<usize>>) -> Self {
        let ny = rows.len();
        let nx = rows.first().map_or(0, Vec::len);
        // Natural order is bottom row first: row r from the top is iy = ny-1-r.
        let mut pattern = Vec::with_capacity(nx * ny);
        let mut ragged = false;
        for iy in 0..ny {
            let row = &rows[ny - 1 - iy];
            ragged |= row.len() != nx;
            for ix in 0..nx {
                pattern.push(row.get(ix).copied().unwrap_or(usize::MAX));
            }
        }
        if ragged {
            // A ragged picture cannot be a grid: make the pattern the wrong
            // length so `assemble` refuses it and says so.
            pattern.push(usize::MAX);
        }
        Self {
            tiles,
            n: [nx, ny, 1],
            pattern,
            covering: None,
        }
    }

    /// Tiles per axis (`[0, 0, 0]` for a [`Self::covering`] lattice before
    /// it is assembled).
    pub fn dimensions(&self) -> [usize; 3] {
        self.n
    }

    /// The distinct tiles.
    pub fn tiles(&self) -> &[Shape] {
        &self.tiles
    }

    /// Per grid position in natural order, the tile index.
    pub fn pattern(&self) -> &[usize] {
        &self.pattern
    }

    /// The tile at `(ix, iy, iz)` from the lower-left, if in range.
    pub fn tile_at(&self, ix: usize, iy: usize, iz: usize) -> Option<&Shape> {
        if ix >= self.n[0] || iy >= self.n[1] || iz >= self.n[2] {
            return None;
        }
        let flat = self.n[0] * self.n[1] * iz + self.n[0] * iy + ix;
        self.pattern.get(flat).and_then(|&t| self.tiles.get(t))
    }

    /// The pitch \[cm\]: the first tile's sides (all must agree), or `None`
    /// when there is no tile or the first is not a box.
    pub fn pitch_cm(&self) -> Option<[f64; 3]> {
        match self.tiles.first().map(|t| &t.solid) {
            Some(Solid::Cuboid(b)) => Some(b.sides_cm()),
            _ => None,
        }
    }

    /// **Why this lattice cannot be built**, or `None`. Checked by
    /// [`super::Model::assemble`].
    pub(crate) fn problem(&self) -> Option<String> {
        if let Some(off) = self.covering {
            if !off.iter().all(|o| o.is_finite()) {
                return Some(format!("a covering lattice's offset must be finite, not {off:?}"));
            }
            if self.tiles.len() != 1 {
                return Some("a covering lattice repeats exactly one tile".into());
            }
        } else if self.n.contains(&0) {
            return Some(format!(
                "a lattice needs at least one tile per axis, not {:?}",
                self.n
            ));
        }
        if self.tiles.is_empty() {
            return Some("a lattice needs at least one tile".into());
        }
        let count = self.n[0] * self.n[1] * self.n[2];
        if self.covering.is_none() && self.pattern.len() != count {
            return Some(format!(
                "a {}x{}x{} lattice needs {count} pattern entries, not {} (rows of different lengths?)",
                self.n[0],
                self.n[1],
                self.n[2],
                self.pattern.len()
            ));
        }
        if let Some(bad) = self
            .pattern
            .iter()
            .find(|&&t| t >= self.tiles.len())
            .filter(|_| self.covering.is_none())
        {
            return Some(format!(
                "the lattice pattern names tile {bad}, but only {} tiles were given",
                self.tiles.len()
            ));
        }
        let Some(pitch) = self.pitch_cm() else {
            return Some("a lattice tile must be a box (Cube::side or Cuboid::sides)".into());
        };
        for t in &self.tiles {
            match &t.solid {
                Solid::Cuboid(b) if b.sides_cm() == pitch && b.centre_cm() == [0.0; 3] => {}
                Solid::Cuboid(b) if b.sides_cm() != pitch => {
                    return Some(format!(
                        "every tile of a lattice must have the same sides: {:?} cm against {:?} cm",
                        b.sides_cm(),
                        pitch
                    ))
                }
                Solid::Cuboid(_) => return Some(
                    "a lattice tile is centred on its own origin; do not move it with centred_at"
                        .into(),
                ),
                Solid::Sphere(_) => {
                    return Some(
                        "a lattice tile must be a box (Cube::side or Cuboid::sides)".into(),
                    )
                }
            }
            if t.boundary.is_some() {
                return Some(
                    "a lattice tile cannot carry a boundary condition; set it on the outermost shape"
                        .into(),
                );
            }
        }
        None
    }

    /// The tile centres \[cm\] when the lattice is centred on `centre`, in
    /// natural order, and the lower-left corner.
    pub(crate) fn layout(&self, pitch: [f64; 3], centre: [f64; 3]) -> ([f64; 3], Vec<[f64; 3]>) {
        let lower: [f64; 3] =
            std::array::from_fn(|i| centre[i] - 0.5 * self.n[i] as f64 * pitch[i]);
        let mut centres = Vec::with_capacity(self.pattern.len());
        for iz in 0..self.n[2] {
            for iy in 0..self.n[1] {
                for ix in 0..self.n[0] {
                    let i = [ix, iy, iz];
                    centres.push(std::array::from_fn(|k| {
                        lower[k] + (i[k] as f64 + 0.5) * pitch[k]
                    }));
                }
            }
        }
        (lower, centres)
    }

    /// **The grid of a [`Self::covering`] lattice** over a sphere of radius
    /// `radius` centred at `centre`: tiles per axis and the lower-left
    /// corner. Per axis, the tiles whose centres sit at `(k + offset) *
    /// pitch` and together cover `[-radius, radius]`. Ported from
    /// `nee_soon::htr10_rmc::core_shell::TrisoBuild::new`'s `triso_axis`
    /// (the HTR-10 core's TRISO lattice, gh:#316/#430), expression for
    /// expression, so the two give the same bits.
    pub(crate) fn covering_grid(
        radius: f64,
        pitch: [f64; 3],
        offset: [f64; 3],
        centre: [f64; 3],
    ) -> ([usize; 3], [f64; 3]) {
        let axis = |p: f64, off: f64| -> (usize, f64) {
            let k_lo = (-radius / p - off + 0.5).floor();
            let k_hi = (radius / p - off - 0.5).ceil();
            ((k_hi - k_lo) as usize + 1, (k_lo + off - 0.5) * p)
        };
        let a: [(usize, f64); 3] = std::array::from_fn(|i| axis(pitch[i], offset[i]));
        (
            a.map(|(n, _)| n),
            std::array::from_fn(|i| a[i].1 + centre[i]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::isotopes::{C12, U235};
    use crate::model::material::{atoms_per_barn_cm, Material};
    use crate::model::shape::{Cube, Cuboid, Sphere};
    use uom::si::f64::Length;
    use uom::si::length::centimeter;

    fn cm(x: f64) -> Length {
        Length::new::<centimeter>(x)
    }

    fn tile(fuel: bool) -> Shape {
        let g = Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]);
        let t = Cube::side(cm(1.0)).filled_with(g);
        if fuel {
            t.containing(
                Sphere::radius(cm(0.3)).filled_with(Material::from_atom_densities([(
                    U235,
                    atoms_per_barn_cm(0.04),
                )])),
            )
        } else {
            t
        }
    }

    /// The two orders name the same grid: a 2x2 picture drawn top-down is
    /// the natural order with its rows swapped (the LCT-008 trap, #926).
    #[test]
    fn rows_top_down_is_natural_order_with_rows_reversed() {
        let top_down = Lattice::from_rows_top_down(
            vec![tile(false), tile(true)],
            vec![vec![1, 0], vec![0, 0]],
        );
        let natural =
            Lattice::from_natural_order(vec![tile(false), tile(true)], [2, 2, 1], vec![0, 0, 1, 0]);
        assert_eq!(top_down, natural);
        assert_eq!(top_down.dimensions(), [2, 2, 1]);
        assert_eq!(top_down.pattern(), &[0, 0, 1, 0]);
        // The fuel tile is top-left: x = 0, y = 1 from the lower-left.
        assert!(!top_down.tile_at(0, 1, 0).unwrap().contained().is_empty());
        assert!(top_down.tile_at(1, 1, 0).unwrap().contained().is_empty());
        assert!(top_down.tile_at(2, 0, 0).is_none());
        assert_eq!(top_down.pitch_cm(), Some([1.0; 3]));
        assert!(top_down.problem().is_none());
        let (lower, centres) = top_down.layout([1.0; 3], [0.0; 3]);
        assert_eq!(lower, [-1.0, -1.0, -0.5]);
        assert_eq!(centres[2], [-0.5, 0.5, 0.0]);
        let c = Lattice::copies_of(tile(true), [2, 3, 4]);
        assert_eq!((c.pattern().len(), c.tiles().len()), (24, 1));
    }

    /// Every malformed lattice is named, never built.
    #[test]
    fn malformed_lattices_are_refused_in_words() {
        let ragged = Lattice::from_rows_top_down(vec![tile(false)], vec![vec![0, 0], vec![0]]);
        assert!(ragged.problem().unwrap().contains("pattern entries"));
        assert!(Lattice::copies_of(tile(false), [0, 1, 1])
            .problem()
            .unwrap()
            .contains("at least one tile per axis"));
        assert!(Lattice::from_natural_order(vec![], [1, 1, 1], vec![0])
            .problem()
            .unwrap()
            .contains("at least one tile"));
        assert!(
            Lattice::from_natural_order(vec![tile(false)], [1, 1, 1], vec![3])
                .problem()
                .unwrap()
                .contains("names tile 3")
        );
        let g = || Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]);
        let ball = Sphere::radius(cm(0.5)).filled_with(g());
        assert!(Lattice::copies_of(ball, [1, 1, 1])
            .problem()
            .unwrap()
            .contains("must be a box"));
        let other = Cuboid::sides(cm(1.0), cm(1.0), cm(2.0)).filled_with(g());
        assert!(
            Lattice::from_natural_order(vec![tile(false), other], [2, 1, 1], vec![0, 1])
                .problem()
                .unwrap()
                .contains("same sides")
        );
        let moved = Cube::side(cm(1.0))
            .centred_at(cm(1.0), cm(0.0), cm(0.0))
            .filled_with(g());
        assert!(Lattice::copies_of(moved, [1, 1, 1])
            .problem()
            .unwrap()
            .contains("centred on its own origin"));
        let bc = tile(false).boundary(crate::model::shape::Boundary::Reflective);
        assert!(Lattice::copies_of(bc, [1, 1, 1])
            .problem()
            .unwrap()
            .contains("boundary"));
    }
}
