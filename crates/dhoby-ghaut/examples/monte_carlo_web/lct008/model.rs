//! The LCT-008 rung's model: ICSBEP LEU-COMP-THERM-008 case 1, the B&W
//! critical lattice of 2.459 w/o UO₂ rods in borated water.
//!
//! **Nothing is retyped here.** The geometry and the materials are the
//! committed `mit-crpg/benchmarks` OpenMC cards, parsed at run time by
//! `outram-mc-libs/examples/common/lct008_model.rs` (pulled in as
//! [`super::spec`]), the same code that `lct008_keff.rs` and the five-route
//! study run. The core is the real one: a 7 × 7 core lattice of 15 × 15 pin
//! lattices, 4961 fuel rods, inside a vacuum-bounded cylinder.
//!
//! **What differs from the recorded result's model, and why.**
//! - **Nuclides**: the 11-nuclide tier (`TAPES_CHEAP`), the tier the quoted
//!   five-route result was measured on. The other 24 (B-11; Mg, Ti, Cr, Fe,
//!   Cu, Zn in the clad) are dropped, not renormalised, as there.
//! - **Data tolerance**: 0.01 ([`SpeedTier::VeryFast`]), not NJOY's 0.001, so
//!   that a browser can process the tapes. That is an approximation. Its
//!   measured effect on this lattice (32 seeds, native) is −65 ± 40 pcm
//!   (`outram-mc-libs/docs/profiling/speed_tiers_2026_09_27.md`). The demo
//!   is Watch-only, an illustration, so no `k` from it is compared with a
//!   record.

use super::spec;
use crate::tapes::read_tape;
use outram_mc_libs::geometry::cell::CellFill;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::Lattice;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use std::collections::BTreeMap;

pub use spec::{R_CLAD, R_CORE, R_FUEL, TEMP_K, Z_HI, Z_LO};

/// The data tier (see the module docs).
pub const SPEED: SpeedTier = SpeedTier::VeryFast;

/// The H in H₂O thermal scattering law, ENDF/B-VIII.0 (MAT 1).
pub const SAB_TAPE: &str = "tsl-HinH2O.endf";

/// Every tape, `(label, file)`: the 11-nuclide tier in `TAPES_CHEAP` order,
/// then the S(α,β) law. Slot `i < 11` is nuclide `i`.
pub const JOBS: [(&str, &str); 12] = [
    ("H-1", spec::TAPES_CHEAP[0].1),
    ("B-10", spec::TAPES_CHEAP[1].1),
    ("O-16", spec::TAPES_CHEAP[2].1),
    ("U-234", spec::TAPES_CHEAP[3].1),
    ("U-235", spec::TAPES_CHEAP[4].1),
    ("U-238", spec::TAPES_CHEAP[5].1),
    ("Al-27", spec::TAPES_CHEAP[6].1),
    ("Si-28", spec::TAPES_CHEAP[7].1),
    ("Si-29", spec::TAPES_CHEAP[8].1),
    ("Si-30", spec::TAPES_CHEAP[9].1),
    ("Mn-55", spec::TAPES_CHEAP[10].1),
    ("H in H2O S(a,b)", SAB_TAPE),
];
const N_NUCLIDES: usize = 11;

/// Index of H-1 and U-235 in the nuclide list.
pub const N_H1: usize = 0;
pub const N_U235: usize = 4;
/// Index of U-238 (the σ(E) panel's capture curve).
pub const N_U238: usize = 5;

/// Material slots in the case-1 cards (asserted when built).
pub const MAT_WATER: usize = 0;
pub const MAT_FUEL: usize = 1;
pub const MAT_CLAD: usize = 2;

/// Incremental processing, one job per call.
pub struct DataBuilder {
    nuclides: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl Default for DataBuilder {
    fn default() -> Self {
        Self { nuclides: Vec::new(), sab: None }
    }
}

impl DataBuilder {
    /// Process the next job from its (covariance-stripped) tape bytes:
    /// RECONR + BROADR to [`TEMP_K`] (URR tables and DBRC on, the library
    /// default), or THERMR for the S(α,β) law.
    /// The expensive half comes from `store` when it holds it (gh:#818).
    pub fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        let i = self.nuclides.len() + usize::from(self.sab.is_some());
        let (label, _) = *JOBS.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, label)?;
        if i < N_NUCLIDES {
            let name = spec::TAPES_CHEAP[i].0;
            let n = store.nuclide(bytes, &tape, mat, name, TEMP_K, SPEED, label)?;
            self.nuclides.push(n);
        } else {
            let s = store.thermal(bytes, &tape, mat, TEMP_K, "c_H_in_H2O", label)?;
            self.sab = Some(s);
        }
        Ok(())
    }

    pub fn finish(self) -> Result<NuclearData, String> {
        if self.nuclides.len() != N_NUCLIDES {
            return Err(format!("{} of {} nuclides done", self.nuclides.len(), N_NUCLIDES));
        }
        let sab = self.sab.ok_or("no S(a,b)")?;
        let mut nuclides = self.nuclides;
        nuclides[N_H1] = nuclides[N_H1].clone().with_thermal_scattering(sab);
        let materials = materials()?;
        Ok(NuclearData { nuclides, materials })
    }
}

pub struct NuclearData {
    pub nuclides: Vec<Nuclide>,
    pub materials: Vec<Material>,
}

/// The case-1 materials on the 11-nuclide tier, built by the shared model
/// code from the committed `materials.xml`.
pub fn materials() -> Result<Vec<Material>, String> {
    let _ = spec::ACTIVE_CASE.set(1);
    let parsed = spec::parse_materials(spec::MATERIALS_XML_CASE1);
    let slots: BTreeMap<String, usize> =
        spec::TAPES_CHEAP.iter().enumerate().map(|(i, (n, _))| (n.to_string(), i)).collect();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in &parsed {
        for (n, ao) in &m.nuclides {
            if !slots.contains_key(n) {
                *omitted.entry(n.clone()).or_insert(0.0) += ao;
            }
        }
    }
    let (mats, clad) = spec::build_materials(&parsed, &slots, &omitted, false);
    let ids: Vec<i32> = mats.iter().map(|m| m.id).collect();
    if ids != [1, 2, 3] || clad != MAT_CLAD {
        return Err(format!("case-1 materials changed: ids {ids:?}, clad slot {clad}"));
    }
    Ok(mats)
}

/// The assembled core: the committed geometry cards, lattice rows in OpenMC's
/// top-down order (`reverse_rows = true`, as `lct008_keff.rs`).
pub fn build_geometry(materials: &[Material]) -> Geometry {
    let _ = spec::ACTIVE_CASE.set(1);
    spec::build_geometry(materials, true)
}

/// What a lattice element holds, for drawing and for births.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinKind {
    Fuel,
    /// Any other non-water pin (none in case 1; pyrex in case 8).
    Other,
}

/// Every non-water pin's centre (x, y), cm, read back from the ASSEMBLED
/// geometry: the core lattice's tile centres plus each assembly lattice's
/// element centres (`RectLattice::tile_center`, the same offsets `locate`
/// subtracts). Water elements are skipped.
pub fn pins(geom: &Geometry) -> Vec<(f64, f64, PinKind)> {
    let rect = |l: usize| match &geom.lattices[l] {
        Lattice::Rect(r) => r,
        Lattice::Hex(_) => panic!("LCT-008 has no hexagonal lattice"),
    };
    let fill_lattice = |u: usize| {
        geom.universes[u].cell_indices.iter().find_map(|&c| match geom.cells[c].fill {
            CellFill::Lattice(l) => Some(l),
            _ => None,
        })
    };
    let kind = |u: usize| {
        let mats: Vec<usize> = geom.universes[u]
            .cell_indices
            .iter()
            .filter_map(|&c| match geom.cells[c].fill {
                CellFill::Material(m) => Some(m),
                _ => None,
            })
            .collect();
        if mats.contains(&MAT_FUEL) {
            Some(PinKind::Fuel)
        } else if mats.iter().all(|&m| m == MAT_WATER) {
            None
        } else {
            Some(PinKind::Other)
        }
    };
    let core = rect(fill_lattice(geom.root_universe).expect("root cell holds the core lattice"));
    let mut out = Vec::new();
    for cy in 0..core.n[1] {
        for cx in 0..core.n[0] {
            let tile = core.universes[core.n[0] * cy + cx];
            let Some(al) = fill_lattice(tile) else { continue }; // the all-water tile
            let c = core.tile_center([cx as i32, cy as i32, 0]);
            let asm = rect(al);
            for ay in 0..asm.n[1] {
                for ax in 0..asm.n[0] {
                    let pu = asm.universes[asm.n[0] * ay + ax];
                    if let Some(k) = kind(pu) {
                        let p = asm.tile_center([ax as i32, ay as i32, 0]);
                        out.push((c.x + p.x, c.y + p.y, k));
                    }
                }
            }
        }
    }
    out
}
