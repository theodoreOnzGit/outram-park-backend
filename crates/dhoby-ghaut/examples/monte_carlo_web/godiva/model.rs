//! The Godiva rung's model: ICSBEP HEU-MET-FAST-001, a bare sphere of highly
//! enriched uranium metal, r = 8.7407 cm.
//!
//! **Nothing is retyped here.** Radius, temperature, the three nuclides, their
//! tapes and atom densities all come from [`outram_mc_libs::vv::godiva`], the
//! model `examples/godiva_keff_endf_local.rs` runs, i.e. the model the
//! recorded result (~~route 4 of the five-route study, −52 ± 27 pcm~~,
//! superseded 2026-10-05 by `godiva_keff_ensemble.rs`'s 1024 seeds, −6 ± 5 pcm,
//! #546) was measured on.
//!
//! **Data tier.** Run k_eff processes the tapes at NJOY's tolerance 0.001
//! ([`SpeedTier::Fast`], the library default and exactly the record's data),
//! so a reader's `k` is comparable with the record (maintainer decision (a),
//! 2026-10-04). Watch mode is an illustration and may use the loosened 0.01
//! tier ([`SpeedTier::VeryFast`]), whose measured effect on Godiva is
//! +7 ± 41 pcm (`outram-mc-libs/docs/profiling/speed_tiers_2026_09_27.md`).

use crate::tapes::read_tape;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::vv::godiva;

pub use godiva::{RADIUS_CM, TEMPERATURE_K};

/// The three tapes, `(label shown while loading, file)`, in
/// [`godiva::NUCLIDES`] order.
pub const JOBS: [(&str, &str); 3] = [
    ("U-234", godiva::NUCLIDES[0].1),
    ("U-235", godiva::NUCLIDES[1].1),
    ("U-238", godiva::NUCLIDES[2].1),
];

/// Index of U-235 in the nuclide list (for the Watch mode's birth spectrum).
pub const N_U235: usize = 1;

/// Incremental processing, one [`Job`] per call.
pub struct DataBuilder {
    tier: SpeedTier,
    nuclides: Vec<Nuclide>,
}

impl DataBuilder {
    pub fn new(tier: SpeedTier) -> Self {
        Self { tier, nuclides: Vec::new() }
    }
    /// Process the next job from its (covariance-stripped) tape bytes, as
    /// `godiva_keff_endf_local.rs` does from the file: RECONR + BROADR to
    /// [`TEMPERATURE_K`] at this builder's tier.
    pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        let i = self.nuclides.len();
        let (name, _, _) = *godiva::NUCLIDES.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, name)?;
        let n = Nuclide::from_tape_with_speed(&tape, mat, name, TEMPERATURE_K, self.tier)
            .map_err(|e| format!("{name}: {e}"))?;
        self.nuclides.push(n);
        Ok(())
    }
    pub fn finish(self) -> Result<NuclearData, String> {
        if self.nuclides.len() != JOBS.len() {
            return Err(format!("{} of {} jobs done", self.nuclides.len(), JOBS.len()));
        }
        Ok(NuclearData { nuclides: self.nuclides, material: godiva::material() })
    }
}

pub struct NuclearData {
    pub nuclides: Vec<Nuclide>,
    pub material: Material,
}

/// The sphere as a CSG geometry, for the Watch mode's traced histories
/// (`run_fixed_source_traced` needs a `Geometry`; the Run mode's
/// `PowerIteration` builds its own sphere from the same radius). One sphere,
/// vacuum outside, one cell of material 0 at [`TEMPERATURE_K`].
pub fn build_geometry() -> Geometry {
    let surfaces = vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: RADIUS_CM, bc: BoundaryType::Vacuum })];
    let inside = vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }];
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, inside, 0, TEMPERATURE_K)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}
