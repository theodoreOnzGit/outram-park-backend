//! The model of the `ugraphite` rung (and the data of the `lumped` rung):
//! uranium and graphite at 296 K, ENDF/B-VIII.0.
//!
//! **Nothing is retyped here.** The tapes, the temperature and the
//! compositions are outram-mc-libs' [`outram_mc_libs::vv::ugraphite`], the
//! module `examples/ugraphite_four_factor.rs` (rung 2) and
//! `examples/lumped_ugraphite_kinf.rs` (rung 3) run, so the demo moves the
//! same atoms the recorded results were measured on.
//!
//! - **Composition shown** ([`mixture`]): rung 2's main case, the uranium and
//!   carbon of one HTR-10 fuel pebble smeared over the ball (17 wt% U-235,
//!   `N_C/N_U` = 767.2), whose recorded `k_inf` is 1.56777 ± 0.00081.
//! - **Geometry** ([`build_geometry`]): the same reflective cube
//!   (`homogeneous_cube`, half-width [`HALF_CM`]) the example uses, so a
//!   neutron reaching a wall comes back: an infinite medium.
//! - **Data tier.** Watch mode is an illustration and processes the tapes at
//!   the loosened tolerance 0.01 ([`SpeedTier::VeryFast`]); its measured
//!   effect on the thermal benchmarks is about −60 pcm
//!   (`outram-mc-libs/docs/profiling/speed_tiers_2026_09_27.md`).

use crate::tapes::read_tape;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::fhr_pebble::homogeneous_cube;
use outram_mc_libs::vv::ugraphite as vv;

pub use vv::TEMPERATURE_K;

/// Half-width of the reflective cube, cm: `examples/ugraphite_four_factor.rs`'s
/// `HALF_CM`. Irrelevant to `k_inf`; it only sets how often a track meets a wall.
pub const HALF_CM: f64 = 50.0;

/// The tapes, `(label shown while loading, file)`: the five nuclides in
/// [`vv::TAPES`] order (the order [`vv::Mix::material`] indexes), then the
/// crystalline-graphite S(α,β) law.
pub const JOBS: [(&str, &str); 6] = [
    ("U-234", vv::TAPES[0].1),
    ("U-235", vv::TAPES[1].1),
    ("U-238", vv::TAPES[2].1),
    ("C-12", vv::TAPES[3].1),
    ("C-13", vv::TAPES[4].1),
    ("graphite S(α,β)", vv::GRAPHITE_TSL.0),
];

/// Index of U-235 in the nuclide list (for the Watch mode's birth spectrum).
pub const N_U235: usize = 1;
/// Index of U-238 and of C-12 (the σ(E) panel, gh:#549).
pub const N_U238: usize = 2;
pub const N_C12: usize = 3;
/// Indices of the carbons, which carry the graphite law.
const N_CARBON: std::ops::Range<usize> = 3..5;

/// Incremental processing, one job per call, as `load_nuclides` in
/// `examples/common/ugraphite_common.rs` does from files: RECONR + BROADR to
/// 296 K, URR and DBRC by the constructor's defaults, and the graphite law on
/// C-12 and C-13.
pub struct DataBuilder {
    tier: SpeedTier,
    nuclides: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl DataBuilder {
    pub fn new(tier: SpeedTier) -> Self {
        Self { tier, nuclides: Vec::new(), sab: None }
    }
    pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        let i = self.nuclides.len() + self.sab.is_some() as usize;
        let (label, _) = *JOBS.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, label)?;
        if i < vv::TAPES.len() {
            let name = vv::TAPES[i].0;
            let n = Nuclide::from_tape_with_speed(&tape, mat, name, TEMPERATURE_K, self.tier)
                .map_err(|e| format!("{label}: {e}"))?;
            self.nuclides.push(n);
        } else {
            let law = ThermalScattering::from_tape(&tape, mat, TEMPERATURE_K, "c_Graphite")
                .map_err(|e| format!("{label}: {e}"))?;
            self.sab = Some(law);
        }
        Ok(())
    }
    /// The five nuclides in [`vv::TAPES`] order, graphite law on the carbons.
    pub fn finish(mut self) -> Result<Vec<Nuclide>, String> {
        let sab = self.sab.take().ok_or("the graphite S(a,b) job was not done")?;
        if self.nuclides.len() != vv::TAPES.len() {
            return Err(format!("{} of {} nuclides done", self.nuclides.len(), vv::TAPES.len()));
        }
        let carbons: Vec<Nuclide> = self.nuclides.drain(N_CARBON).map(|n| n.with_thermal_scattering(sab.clone())).collect();
        self.nuclides.extend(carbons);
        Ok(self.nuclides)
    }
}

/// Rung 2's main case: one HTR-10 fuel pebble's uranium and carbon.
pub fn mixture() -> vv::Mix {
    vv::htr10_pebble_mix().0
}

/// The one material, at index 0.
pub fn materials() -> Vec<Material> {
    vec![mixture().material(1, "U(17 wt%) + graphite, HTR-10 pebble ratio")]
}

/// The reflective cube filled with material 0.
pub fn build_geometry() -> Geometry {
    homogeneous_cube(HALF_CM, 0, TEMPERATURE_K)
}
