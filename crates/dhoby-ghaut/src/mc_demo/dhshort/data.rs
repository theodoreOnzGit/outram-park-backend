//! The `dhshort` rung's nuclear data, for its live run: the ten
//! nuclides of `dh_keff_vv.rs`'s `nuclides()`, in its order (the material
//! table's indices, `fhr::U235` … `fhr::F19`), from the same ENDF/B-VIII.0
//! tapes at the same temperature (600 K) and NJOY tolerance (0.001), and the
//! ENDF/B-VIII.0 crystalline-graphite S(α,β) law (MAT 30) on the graphite
//! carbon only, as the record does. C-12 is processed once and used twice:
//! free gas (kernel and SiC carbon) and graphite-bound (everything else).
//!
//! **Per-material binding (GitHub #462, #1635 item 3, 2026-11-17).** The table
//! holds every slot free, the graphite slot (`fhr::C_GRAPHITE`) a free clone of
//! C-12; the law goes to the five materials that reference that slot through
//! `assign_material_sab`, as `examples/dh_keff_vv.rs` does, which appends a
//! bound copy (index 10) and repoints those materials at it. The bound
//! materials come back beside the table and are what the live universe is
//! built with ([`super::model::build_with`]). Compile-checked only: a browser
//! demo, not run.
//!
//! The Watch view needs none of this; only the k∞ view loads it (the full
//! tier, like the `htr10` rung's fuel zone).

use super::model::fhr;
use crate::mc_demo::engine::Tier;
use crate::mc_demo::tapes::read_tape;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::material_sab::{assign_material_sab, MaterialSab};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;

/// The tapes, in processing order: `(label, file, name, index in the nuclide
/// table)`; the graphite law last. C-12 fills two indices.
pub const JOBS: [(&str, &str); 10] = [
    ("U-235", "n-092_U_235-ENDF8.0.endf"),
    ("U-238", "n-092_U_238.endf"),
    ("O-16", "n-008_O_016-ENDF8.0.endf"),
    ("C-12", "n-006_C_012-ENDF8.0.endf"),
    ("Si-28", "n-014_Si_028-ENDF8.0.endf"),
    ("Li-6", "n-003_Li_006-ENDF8.0.endf"),
    ("Li-7", "n-003_Li_007-ENDF8.0.endf"),
    ("Be-9", "n-004_Be_009-ENDF8.0.endf"),
    ("F-19", "n-009_F_019-ENDF8.0.endf"),
    ("graphite S(α,β)", "tsl-crystalline-graphite.endf"),
];

/// `dh_keff_vv`'s nuclide names, per job (the graphite law has none).
const NAMES: [&str; 9] = [
    "U235", "U238", "O16", "C12", "Si28", "Li6", "Li7", "Be9", "F19",
];

/// The graphite law's MAT number in `tsl-crystalline-graphite.endf`.
pub const GRAPHITE_MAT: i32 = 30;

/// One tape at a time, so the page shows progress between the long jobs.
pub struct DataBuilder {
    tier: Tier,
    done: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl DataBuilder {
    pub fn new(tier: Tier) -> Self {
        Self {
            tier,
            done: Vec::new(),
            sab: None,
        }
    }

    /// RECONR + BROADR at 600 K and tolerance 0.001 ([`SpeedTier::Fast`]),
    /// or THERMR for the graphite law.
    /// The expensive half comes from `store` when it holds it (gh:#818).
    pub fn step(
        &mut self,
        bytes: &[u8],
        store: &mut crate::mc_demo::processed_cache::DataStore,
    ) -> Result<(), String> {
        let i = self.done.len() + usize::from(self.sab.is_some());
        match Self::make(i, bytes, store)? {
            crate::mc_demo::rungs::Product::Nuclide(n) => self.done.push(n),
            crate::mc_demo::rungs::Product::Law(l) => self.sab = Some(l),
        }
        Ok(())
    }

    /// Job `i`'s product (what [`Self::step`] keeps, and a processing-pool
    /// worker only caches).
    pub fn make(i: usize, bytes: &[u8], store: &mut crate::mc_demo::processed_cache::DataStore) -> Result<crate::mc_demo::rungs::Product, String> {
        let (label, _) = *JOBS.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, label)?;
        if i < NAMES.len() {
            Ok(crate::mc_demo::rungs::Product::Nuclide(store.nuclide(bytes, &tape, mat, NAMES[i], fhr::TEMP_K, SpeedTier::Fast, label)?))
        } else {
            if mat != GRAPHITE_MAT {
                return Err(format!("{label}: MAT {mat}, expected {GRAPHITE_MAT}"));
            }
            Ok(crate::mc_demo::rungs::Product::Law(store.thermal(bytes, &tape, mat, fhr::TEMP_K, "c_Graphite", label)?))
        }
    }

    /// The nuclide table in `fhr`'s index order with the graphite law bound
    /// per material (bound C-12 appended), and the materials repointed at it,
    /// if this load processed the tapes (the full tier); `None` for the Watch
    /// view's load.
    pub fn finish(self) -> Result<Option<(Vec<Material>, Vec<Nuclide>)>, String> {
        if self.tier == Tier::NoData {
            return Ok(None);
        }
        let sab = self.sab.ok_or("no graphite S(α,β)")?;
        if self.done.len() != NAMES.len() {
            return Err(format!(
                "{} of {} nuclides processed",
                self.done.len(),
                NAMES.len()
            ));
        }
        let mut it = self.done.into_iter();
        let mut next = || it.next().ok_or("nuclide missing");
        let (u235, u238, o16, c12) = (next()?, next()?, next()?, next()?);
        // The graphite slot is a free clone; the law is bound per material below.
        let graphite_slot = c12.clone();
        let rest = [next()?, next()?, next()?, next()?, next()?];
        let mut table = vec![u235, u238, o16, c12, graphite_slot];
        table.extend(rest);
        // The indices the material table uses (`fhr::U235` … `fhr::F19`).
        debug_assert_eq!(
            (fhr::C_FREE, fhr::C_GRAPHITE, fhr::SI28, fhr::F19),
            (3, 4, 5, 9)
        );
        let mats = fhr::materials();
        let entries = MaterialSab::for_materials_containing(&mats, fhr::C_GRAPHITE, 0, &[]);
        assign_material_sab(&mats, &table, &[sab], &entries).map(Some)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The table comes out in `fhr`'s index order, with the graphite law on
    /// index 4 only, from the tapes the record reads. Opt-in: it processes
    /// ten tapes at tolerance 0.001 (minutes).
    #[test]
    #[ignore = "processes 10 ENDF tapes at tolerance 0.001 (minutes); run with --ignored"]
    fn the_table_is_dh_keff_vvs_order() {
        let mut b = DataBuilder::new(Tier::Exact);
        for (label, tape) in JOBS {
            let raw = std::fs::read(
                njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape"),
            )
            .expect("read");
            let t = std::time::Instant::now();
            b.step(&crate::mc_demo::tapes::strip_covariances(&raw), &mut crate::mc_demo::processed_cache::DataStore::off())
                .expect("step");
            eprintln!("  {label:<16} {:6.1} s", t.elapsed().as_secs_f64());
        }
        let (mats, n) = b.finish().unwrap().unwrap();
        assert_eq!(n.len(), 11, "ten slots and the bound graphite carbon");
        // The five graphite materials reference the bound copy, the kernel and
        // SiC keep the free C-12, and the graphite slot itself is unreferenced.
        let at = |i: usize| n[i].total_at_energy(0.0253, fhr::TEMP_K);
        assert!(n[10].has_thermal_scattering() && !n[fhr::C_GRAPHITE].has_thermal_scattering());
        assert!(at(10) != at(fhr::C_FREE), "graphite S(a,b) not attached");
        for (i, m) in mats.iter().enumerate() {
            for c in &m.components {
                assert_ne!(c.nuclide_idx, fhr::C_GRAPHITE, "material {i} still holds the free graphite slot");
            }
        }
        assert!(mats.iter().any(|m| m.components.iter().any(|c| c.nuclide_idx == 10)));
    }

    /// A Watch load processes nothing and holds no table.
    #[test]
    fn a_watch_load_has_no_table() {
        assert!(DataBuilder::new(Tier::NoData).finish().unwrap().is_none());
        assert!(DataBuilder::new(Tier::Exact).finish().is_err());
    }
}
