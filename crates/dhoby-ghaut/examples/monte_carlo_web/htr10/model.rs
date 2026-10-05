//! The `htr10` rung's models. **Nothing is re-modelled here.**
//!
//! - **The core** is `nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14,
//!   N, 0)`, the builder the recorded k-vs-height runs used
//!   (`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_01_10k/`)
//!   and the geometry review images were drawn from
//!   (`htr10_geometry_images/`): Şeker & Çolak (2003)'s 13-ball lattice bed of
//!   N layers, every pebble whole, TRISO particles on a lattice in each fuel
//!   pebble, the reflector, rod channels and cavity. Built in the worker on the
//!   first slice asked for (0.2 s natively at N = 12) and rebuilt when N
//!   changes. No nuclear data are needed to draw it.
//! - **The fuel zone** is `outram-mc-libs/examples/common/htr10_fuel_zone.rs`,
//!   pulled in unchanged ([`fz`]): the 1 cm half-width reflective cube of
//!   1018 RSA-packed UO₂ kernels in graphite matrix (IAEA-TECDOC-1382 Table
//!   4-38 densities) against the same atoms homogenised, the case
//!   `htr10_fuel_zone_kinf.rs` records. ENDF/B-VIII.0 at NJOY's tolerance
//!   0.001 (the record's), 293.15 K, URR and DBRC on by default, graphite
//!   `tsl-reactor-graphite-30P` on both carbons. The majorant is
//!   `Majorant::bounding` through the shared file: since #585 (282d22d35) the
//!   library tabulates it on every nuclide breakpoint, a bound by
//!   construction. (~~the union-grid majorant of the record, not
//!   `Majorant::bounding`, which under-bounds here~~: corrected 2026-10-05
//!   when #585 landed; the record used the example's local union-grid
//!   construction, both are bounds. The audit of both arms on this rung's
//!   data is in `the_fuel_zone_case_runs_both_arms`.)

use crate::engine::Tier;
use crate::tapes::read_tape;
use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, AssembledCore};
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;

/// The shared fuel-zone model (densities, materials, majorant, tapes).
#[path = "../../../../outram-mc-libs/examples/common/htr10_fuel_zone.rs"]
#[allow(clippy::all)]
pub mod fz;

/// The record's temperature: benchmark B1 core, 20 °C.
pub const TEMP_K: f64 = 293.15;
/// The record's packing: half-width of the reflective cube, cm, and seed.
pub const HALF_CM: f64 = 1.0;
pub const PACKING_SEED: u64 = 20260811;
/// Rings of the core model (as the recorded runs).
pub const RINGS: usize = 14;

/// Every tape: the seven nuclides of [`fz::ENDF_TAPES`], then the graphite law.
pub const JOBS: [(&str, &str); 8] = [
    ("U-235", fz::ENDF_TAPES[0].1),
    ("U-238", fz::ENDF_TAPES[1].1),
    ("O-16", fz::ENDF_TAPES[2].1),
    ("C-12", fz::ENDF_TAPES[3].1),
    ("C-13", fz::ENDF_TAPES[4].1),
    ("B-10", fz::ENDF_TAPES[5].1),
    ("B-11", fz::ENDF_TAPES[6].1),
    ("graphite S(α,β)", fz::GRAPHITE_TSL_30P.0),
];

pub struct DataBuilder {
    tier: Tier,
    nuclides: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl DataBuilder {
    pub fn new(tier: Tier) -> Self {
        Self { tier, nuclides: Vec::new(), sab: None }
    }
    /// The next tape: RECONR + BROADR at [`TEMP_K`], tolerance 0.001, or the
    /// graphite law (THERMR).
    pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        let i = self.nuclides.len() + usize::from(self.sab.is_some());
        let (label, _) = *JOBS.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, label)?;
        if i < fz::ENDF_TAPES.len() {
            let n = Nuclide::from_tape_with_speed(&tape, mat, fz::ENDF_TAPES[i].0, TEMP_K, SpeedTier::Fast)
                .map_err(|e| format!("{label}: {e}"))?;
            self.nuclides.push(n);
        } else {
            self.sab = Some(ThermalScattering::from_tape(&tape, mat, TEMP_K, "c_Graphite").map_err(|e| format!("{label}: {e}"))?);
        }
        Ok(())
    }
    /// The fuel zone, if this load processed the tapes (the full tier).
    pub fn finish(self) -> Result<Option<FuelZone>, String> {
        if self.tier == Tier::Loose {
            return Ok(None);
        }
        let sab = self.sab.ok_or("no graphite S(a,b)")?;
        if self.nuclides.len() != fz::ENDF_TAPES.len() {
            return Err(format!("{} of {} nuclides processed", self.nuclides.len(), fz::ENDF_TAPES.len()));
        }
        let nuclides: Vec<Nuclide> = self
            .nuclides
            .into_iter()
            .enumerate()
            .map(|(i, n)| if i == 3 || i == 4 { n.with_thermal_scattering(sab.clone()) } else { n })
            .collect();
        FuelZone::new(nuclides).map(Some)
    }
}

/// The fuel-zone cube, kernels resolved and homogenised.
pub struct FuelZone {
    pub nuclides: Vec<Nuclide>,
    /// Kernel (0) and matrix (1).
    pub het: Vec<Material>,
    pub hom: Vec<Material>,
    pub packed: PackedSpheres,
    /// Built on the first run of each arm (they take seconds).
    pub maj_het: Option<Majorant>,
    pub maj_hom: Option<Majorant>,
}

impl FuelZone {
    pub fn new(nuclides: Vec<Nuclide>) -> Result<Self, String> {
        let f = fz::kernel_packing_fraction();
        let (kernel, matrix, hom) = fz::build_materials(&fz::endf_layout(), TEMP_K, f);
        let packed = PackedSpheres::pack(fz::KERNEL_RADIUS_CM, HALF_CM, f, PACKING_SEED).map_err(|e| format!("packing: {e:?}"))?;
        Ok(Self { nuclides, het: vec![kernel, matrix], hom: vec![hom], packed, maj_het: None, maj_hom: None })
    }
    /// The arm's materials and majorant (building the majorant if needed).
    pub fn arm(&mut self, resolved: bool) -> (&[Material], &Majorant, &[Nuclide], &PackedSpheres) {
        let (mats, maj) = if resolved { (&self.het, &mut self.maj_het) } else { (&self.hom, &mut self.maj_hom) };
        if maj.is_none() {
            *maj = Some(fz::build_majorant(mats, &self.nuclides));
        }
        (mats.as_slice(), maj.as_ref().expect("built above"), &self.nuclides, &self.packed)
    }
}

/// The core at `layers` Şeker layers.
pub fn core(layers: usize) -> AssembledCore {
    assemble_explicit_triso(RINGS, layers.clamp(10, 20), 0)
}
