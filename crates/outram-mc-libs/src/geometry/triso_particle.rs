//! Full nested-shell TRISO fuel-particle geometry (kernel + 4 coatings).
//!
//! A real TRISO (TRi-structural ISOtropic) fuel particle is **five concentric
//! regions**, not the single fuel sphere the `triso` notebook verification test
//! collapses into the matrix. From the centre outward:
//!
//! 1. **fuel kernel** — the fissile ceramic microsphere (UO2 for HTR-10),
//! 2. **buffer** — a low-density porous pyrolytic-carbon layer that gives fission
//!    gases somewhere to go and absorbs fuel-kernel swelling / recoils,
//! 3. **IPyC** — inner (dense) pyrolytic carbon, the inner seal coat,
//! 4. **SiC** — silicon carbide, the primary pressure boundary / metallic-fission-
//!    product barrier (the structural "miniature pressure vessel"),
//! 5. **OPyC** — outer pyrolytic carbon, the outer bonding / protective coat,
//!
//! all embedded in a graphite **matrix** (the compact / pebble binder) that fills
//! the rest of the particle's universe.
//!
//! This module **assembles existing CSG primitives** — it is not an OpenMC port.
//! It places five concentric [`Sphere`] surfaces at the cumulative outer radii of
//! each region and builds one [`Cell`] per shell as the half-space region between
//! consecutive spheres (`+sense` of the inner sphere ∧ `−sense` of the outer),
//! with the kernel as the innermost `−sense` ball and the matrix as the outermost
//! `+sense` exterior. The result is a [`Universe`] a lattice tile or a bounding
//! cell can be filled with, exactly like the single-kernel universe the existing
//! `triso` test builds — only now with the real coating stack resolved.
//!
//! # Reference dimensions (typical HTR-10 / reference TRISO — NOT an authoritative spec)
//!
//! The [`TrisoRadii::HTR10`] preset uses the widely cited HTR-10 pebble-bed TRISO
//! geometry (UO2 kernel + the standard four coatings). These are **typical /
//! reference** values drawn from open pebble-bed-HTGR literature, provided as a
//! convenience default — they are not a controlled design specification and must
//! not be treated as one:
//!
//! | Region | Layer size | Cumulative outer radius |
//! |---|---|---|
//! | UO2 kernel   | 250 µm radius   | 0.0250 cm |
//! | buffer (PyC) | 95 µm thick     | 0.0345 cm |
//! | IPyC         | 40 µm thick     | 0.0385 cm |
//! | SiC          | 35 µm thick     | 0.0420 cm |
//! | OPyC         | 40 µm thick     | 0.0460 cm |
//!
//! (1 µm = 1e-4 cm; the kernel figure is a *radius*, the four coatings are
//! *thicknesses* accumulated onto it.) The builder itself is fully general — it
//! takes whatever five cumulative radii the caller supplies; the preset only
//! encodes the table above.
//!
//! # Scope: geometry + material assignment only
//!
//! Nothing here loads or asserts any thermal data; the material ids are opaque
//! indices the caller maps to real materials. That is deliberate and stays that
//! way — deck loading in a CSG module would break the crate's data/transport
//! boundary (`outram-mc-libs` parses no ENDF; it consumes the njoy surface).
//!
//! **S(α,β) thermal scattering is therefore attached caller-side**, to the
//! [`Nuclide`](crate::material::nuclide::Nuclide)s that make up the materials
//! passed in here, via `Nuclide::with_thermal_scattering`. It is no longer
//! blocked on data availability: as of 2026-08-14 the ENDF/B-VIII.0 LEAPR decks
//! are embedded in `njoy-outram-park-fork` and regenerate offline. See
//! `tests/triso_shell_thermal_scattering.rs` for the worked, checked
//! composition, and note its finding:
//!
//! - **PyC coatings and matrix (carbon in graphite) — available and verified.**
//! - **SiC layer — available as of 2026-08-19, not yet fully verified.** Stock
//!   LEAPR could not generate SiC's coherent-elastic channel (card 4
//!   `iel = 0`); a generalized coherent-elastic implementation (bead
//!   `op-jw4a`, mirrors GitHub issue #24) now produces a real MF=7/MT=2
//!   channel for both `tsl-CinSiC` and `tsl-SiinSiC`, measured within ~3% of
//!   the official ENDF/B-VIII.0 tape oracle at 0.0253 eV
//!   (`crates/njoy-outram-park-fork/tests/leapr_sic_coherent_elastic_oracle.rs`).
//!   **Still do not substitute the graphite law for the SiC layer** — it
//!   remains a different lattice. **Still do not sum both SiC materials'
//!   elastic channels for one region** — coherent elastic is a property of
//!   the 3C-SiC compound as a whole and both materials carry the identical
//!   value; a caller must attribute MT=2 to the compound once. Remaining
//!   follow-up (tracked in `op-jw4a`, not yet done): a tighter-tolerance
//!   edge-by-edge validation, and root-causing the residual ~3% gap.

use super::cell::{Cell, HalfSpaceSense, RegionToken};
use super::geometry::Geometry;
use super::position::Position;
use super::surface::{BoundaryType, Sphere, SurfaceKind};
use super::universe::Universe;

/// The five cumulative **outer radii** \[cm\] of a TRISO particle's regions.
///
/// Each field is the radius of the *outer* boundary of that region (measured from
/// the particle centre), so they must be **strictly increasing**:
/// `kernel < buffer < ipyc < sic < opyc`. A region's radial thickness is the
/// difference between its outer radius and the previous one (the kernel's
/// "thickness" is just its radius).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrisoRadii {
    /// Fuel-kernel outer radius \[cm\].
    pub kernel: f64,
    /// Buffer outer radius \[cm\] (kernel radius + buffer thickness).
    pub buffer: f64,
    /// IPyC outer radius \[cm\].
    pub ipyc: f64,
    /// SiC outer radius \[cm\].
    pub sic: f64,
    /// OPyC outer radius \[cm\] — the overall particle radius.
    pub opyc: f64,
}

impl TrisoRadii {
    /// Typical HTR-10 / reference-TRISO cumulative outer radii \[cm\]: 250 µm UO2
    /// kernel radius, then 95 / 40 / 35 / 40 µm buffer / IPyC / SiC / OPyC
    /// thicknesses (see the module-level table). **Reference values, not an
    /// authoritative specification.**
    pub const HTR10: Self = Self {
        kernel: 0.0250,
        buffer: 0.0345,
        ipyc: 0.0385,
        sic: 0.0420,
        opyc: 0.0460,
    };

    /// The five outer radii as an array, kernel-first (outermost = index 4).
    #[inline]
    pub fn as_array(self) -> [f64; 5] {
        [self.kernel, self.buffer, self.ipyc, self.sic, self.opyc]
    }

    /// Whether the radii are strictly increasing and positive (a physically valid
    /// nesting).
    #[inline]
    pub fn is_strictly_increasing(self) -> bool {
        let r = self.as_array();
        r[0] > 0.0 && r.windows(2).all(|w| w[0] < w[1])
    }
}

/// Material indices for the five shells plus the surrounding matrix.
///
/// Each field is an opaque index into the caller's global material array (the
/// same convention as [`CellFill::Material`](super::cell::CellFill::Material)).
/// The shells are filled in nesting order; `matrix` fills the rest of the
/// particle's universe outside the OPyC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrisoMaterials {
    /// Fuel-kernel material index.
    pub kernel: usize,
    /// Buffer (porous PyC) material index.
    pub buffer: usize,
    /// IPyC (inner pyrolytic carbon) material index.
    pub ipyc: usize,
    /// SiC (silicon carbide) material index.
    pub sic: usize,
    /// OPyC (outer pyrolytic carbon) material index.
    pub opyc: usize,
    /// Surrounding matrix (graphite compact / pebble binder) material index.
    pub matrix: usize,
}

impl TrisoMaterials {
    /// The five shell material indices, kernel-first (matrix excluded).
    #[inline]
    fn shells(self) -> [usize; 5] {
        [self.kernel, self.buffer, self.ipyc, self.sic, self.opyc]
    }
}

/// One assembled TRISO particle: the concentric surfaces, the per-shell cells,
/// and the universe that searches them.
///
/// The [`Universe::cell_indices`] point at `cells` offset by the `cell_base`
/// passed to [`build_triso_particle`]; each cell's region tokens index `surfaces`
/// offset by `surface_base`. With both bases `0` (the [`triso_particle`]
/// convenience) the vectors are self-contained and can be dropped straight into a
/// [`Geometry`] via [`TrisoParticle::into_geometry`].
pub struct TrisoParticle {
    /// The five concentric sphere surfaces, kernel-first.
    pub surfaces: Vec<SurfaceKind>,
    /// The six cells: kernel, buffer, IPyC, SiC, OPyC, matrix (in that order).
    pub cells: Vec<Cell>,
    /// The universe searching the six cells in nesting order.
    pub universe: Universe,
}

impl TrisoParticle {
    /// Wrap this self-contained particle (built with `surface_base = cell_base =
    /// 0`) as a standalone [`Geometry`] whose root is the particle universe.
    ///
    /// Only valid when the particle was built with both bases `0` (e.g. via
    /// [`triso_particle`]); otherwise the internal indices would not line up with
    /// the fresh single-universe arrays.
    pub fn into_geometry(self) -> Geometry {
        Geometry {
            surfaces: self.surfaces,
            cells: self.cells,
            universes: vec![self.universe],
            lattices: vec![],
            root_universe: 0,
        }
    }
}

/// Build a full nested-shell TRISO particle universe from five radii and six
/// material ids.
///
/// Places five concentric [`Sphere`] surfaces (all [`BoundaryType::Transmissive`]
/// — these are internal material interfaces, not model boundaries) at `center`
/// with the cumulative outer radii in `radii`, then one [`Cell`] per region:
///
/// - **kernel** — `−sense` of sphere 0 (the innermost ball),
/// - **buffer / IPyC / SiC / OPyC** — `+sense` of the previous sphere ∧ `−sense`
///   of this shell's sphere (the annular gap between consecutive spheres),
/// - **matrix** — `+sense` of sphere 4 (everything outside the OPyC), filling the
///   rest of the universe.
///
/// `surface_base` / `cell_base` are the global offsets at which these surfaces /
/// cells will live in the enclosing [`Geometry`]'s flat arrays, so the region
/// tokens and the universe's `cell_indices` refer to the right global slots when
/// this universe is spliced into a larger model (a lattice of particles, a
/// pebble, …). For a standalone particle pass `0` for both (see
/// [`triso_particle`]). `temperature` \[K\] is stamped on every cell.
///
/// # Panics (debug only)
///
/// Debug-asserts that `radii` is strictly increasing; in release, non-increasing
/// radii silently produce empty (unreachable) shells rather than aborting.
pub fn build_triso_particle(
    center: Position,
    radii: TrisoRadii,
    materials: TrisoMaterials,
    temperature: f64,
    universe_id: i32,
    surface_base: usize,
    cell_base: usize,
) -> TrisoParticle {
    debug_assert!(
        radii.is_strictly_increasing(),
        "TRISO radii must be strictly increasing (kernel<buffer<ipyc<sic<opyc), got {radii:?}"
    );

    // Five concentric interface spheres, kernel-first.
    let surfaces: Vec<SurfaceKind> = radii
        .as_array()
        .into_iter()
        .map(|r| {
            SurfaceKind::Sphere(Sphere {
                x0: center.x,
                y0: center.y,
                z0: center.z,
                r,
                bc: BoundaryType::Transmissive,
            })
        })
        .collect();

    // Global surface index of the i-th sphere once spliced in.
    let surf = |i: usize| surface_base + i;
    let inside = |i: usize| RegionToken::HalfSpace {
        surface_idx: surf(i),
        sense: HalfSpaceSense::Inside,
    };
    let outside = |i: usize| RegionToken::HalfSpace {
        surface_idx: surf(i),
        sense: HalfSpaceSense::Outside,
    };

    let shell_mats = materials.shells();
    let mut cells: Vec<Cell> = Vec::with_capacity(6);

    // Cell 0: fuel kernel — the −sense ball of sphere 0.
    cells.push(Cell::material(
        universe_id * 10 + 1,
        vec![inside(0)],
        shell_mats[0],
        temperature,
    ));

    // Cells 1..=4: buffer, IPyC, SiC, OPyC — the annulus between consecutive
    // spheres: +sense(inner) ∧ −sense(outer).
    for i in 1..5 {
        cells.push(Cell::material(
            universe_id * 10 + 1 + i as i32,
            vec![outside(i - 1), inside(i), RegionToken::Intersection],
            shell_mats[i],
            temperature,
        ));
    }

    // Cell 5: matrix — everything outside the OPyC (sphere 4), filling the rest of
    // the universe.
    cells.push(Cell::material(
        universe_id * 10 + 6,
        vec![outside(4)],
        materials.matrix,
        temperature,
    ));

    let universe = Universe {
        id: universe_id,
        cell_indices: (0..6).map(|i| cell_base + i).collect(),
    };

    TrisoParticle {
        surfaces,
        cells,
        universe,
    }
}

/// Convenience: a self-contained TRISO particle centred at `center`
/// (`surface_base = cell_base = 0`, universe id `1`).
///
/// The returned [`TrisoParticle`]'s vectors are internally consistent and can be
/// turned into a standalone [`Geometry`] with [`TrisoParticle::into_geometry`].
pub fn triso_particle(
    center: Position,
    radii: TrisoRadii,
    materials: TrisoMaterials,
    temperature: f64,
) -> TrisoParticle {
    build_triso_particle(center, radii, materials, temperature, 1, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::position::Direction;

    /// Distinct material indices, one per region, so a located material index
    /// uniquely identifies the shell it came from.
    const MATS: TrisoMaterials = TrisoMaterials {
        kernel: 0,
        buffer: 1,
        ipyc: 2,
        sic: 3,
        opyc: 4,
        matrix: 5,
    };

    /// **V&V — nested-shell material-at-point correctness (geometry only).**
    ///
    /// # Methodology
    ///
    /// Build a standalone HTR-10-dimensioned TRISO particle
    /// ([`TrisoRadii::HTR10`], distinct material ids 0..=5 for kernel / buffer /
    /// IPyC / SiC / OPyC / matrix) centred at the origin and turn it into a
    /// [`Geometry`]. Drive [`Geometry::locate`] — the real nested-cell descent the
    /// transport loop uses — at radial sample points along `+x` and assert the
    /// returned **leaf material index** equals the expected region:
    ///
    /// - a point at `r` just inside each of the five interface spheres returns
    ///   that shell's material (kernel, buffer, IPyC, SiC, OPyC respectively) —
    ///   this checks the descent lands in the shell whose *outer* boundary is that
    ///   sphere;
    /// - the mid-radius of each of the five shells returns that shell's material;
    /// - the origin returns the kernel; and
    /// - a point just outside, and a point far outside, the OPyC return the
    ///   matrix.
    ///
    /// Pass criterion: every sampled point resolves to the correct material index.
    /// This is a **geometry** test (CSG half-space membership + universe descent);
    /// it makes **no** claim about neutronics — there is no k-eff, no cross
    /// section, and no thermal-scattering data involved, so none is reported. The
    /// carbon/graphite S(α,β) thermal-scattering integration is explicitly
    /// deferred (see the module docs) and is **not** exercised here.
    ///
    /// # Results (measured 2026-08-12, this harness)
    ///
    /// All sampled points resolve to the expected region: origin and just-inside-
    /// kernel → kernel (0); just-inside and mid buffer / IPyC / SiC / OPyC →
    /// 1 / 2 / 3 / 4; just-outside and far-outside OPyC → matrix (5). The full
    /// five-region nested-shell descent is verified. No physics/data claim is made
    /// (geometry-only).
    #[test]
    fn triso_nested_shell_locate_material_at_point() {
        let radii = TrisoRadii::HTR10;
        assert!(radii.is_strictly_increasing());
        let geom = triso_particle(Position::ZERO, radii, MATS, 293.6).into_geometry();
        let u = Direction::new(1.0, 0.0, 0.0);

        // Locate at radius `r` on the +x axis and return the leaf material.
        let mat_at = |r: f64| -> Option<usize> {
            geom.locate(Position::new(r, 0.0, 0.0), u, usize::MAX)
                .and_then(|p| p.material)
        };

        let rs = radii.as_array(); // [kernel, buffer, ipyc, sic, opyc] outer radii
        let expected = [MATS.kernel, MATS.buffer, MATS.ipyc, MATS.sic, MATS.opyc];

        // Origin → kernel.
        assert_eq!(
            mat_at(0.0),
            Some(MATS.kernel),
            "origin must be the fuel kernel"
        );

        // Just inside each interface sphere → that shell's material.
        for (i, &r_out) in rs.iter().enumerate() {
            let r = r_out * (1.0 - 1.0e-6); // a hair inside sphere i
            assert_eq!(
                mat_at(r),
                Some(expected[i]),
                "point just inside sphere {i} (r={r}) should be shell material {}",
                expected[i]
            );
        }

        // Mid-radius of each shell → that shell's material. Shell 0 spans
        // (0, kernel); shell i spans (r_{i-1}, r_i).
        let mut inner = 0.0;
        for (i, &r_out) in rs.iter().enumerate() {
            let mid = 0.5 * (inner + r_out);
            assert_eq!(
                mat_at(mid),
                Some(expected[i]),
                "mid-radius of shell {i} (r={mid}) should be material {}",
                expected[i]
            );
            inner = r_out;
        }

        // Just outside and far outside the OPyC → matrix.
        let r_opyc = radii.opyc;
        assert_eq!(
            mat_at(r_opyc * (1.0 + 1.0e-6)),
            Some(MATS.matrix),
            "just outside the OPyC should be matrix"
        );
        assert_eq!(
            mat_at(r_opyc + 1.0),
            Some(MATS.matrix),
            "far outside the OPyC should still be matrix"
        );
    }

    /// The assembled particle has exactly five interface spheres and six cells
    /// (kernel + four coatings + matrix), and the universe searches all six.
    #[test]
    fn triso_particle_has_five_spheres_and_six_cells() {
        let particle = triso_particle(Position::ZERO, TrisoRadii::HTR10, MATS, 293.6);
        assert_eq!(
            particle.surfaces.len(),
            5,
            "five concentric interface spheres"
        );
        assert_eq!(particle.cells.len(), 6, "kernel + 4 coatings + matrix");
        assert_eq!(particle.universe.cell_indices, vec![0, 1, 2, 3, 4, 5]);
    }

    /// A non-zero `surface_base` / `cell_base` shifts every region token and every
    /// universe cell index by the offset — the splice-into-a-larger-model path.
    #[test]
    fn triso_particle_respects_index_bases() {
        let particle =
            build_triso_particle(Position::ZERO, TrisoRadii::HTR10, MATS, 293.6, 7, 100, 50);
        assert_eq!(particle.universe.id, 7);
        assert_eq!(
            particle.universe.cell_indices,
            vec![50, 51, 52, 53, 54, 55],
            "cell indices offset by cell_base"
        );
        // The kernel cell's half-space must reference the sphere at surface_base (100).
        let RegionToken::HalfSpace { surface_idx, .. } = particle.cells[0].region[0] else {
            panic!("kernel region should be a single half-space");
        };
        assert_eq!(surface_idx, 100, "region token offset by surface_base");
    }
}
