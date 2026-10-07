//! The `dhshort` rung's nuclear data, for its live low-statistics run: the ten
//! nuclides of `dh_keff_vv.rs`'s `nuclides()`, in its order (the material
//! table's indices, `fhr::U235` … `fhr::F19`), from the same ENDF/B-VIII.0
//! tapes at the same temperature (600 K) and NJOY tolerance (0.001), and the
//! ENDF/B-VIII.0 crystalline-graphite S(α,β) law (MAT 30) on the graphite
//! carbon only, as the record does. C-12 is processed once and used twice:
//! free gas (kernel and SiC carbon) and graphite-bound (everything else).
//!
//! The Watch view needs none of this; only the k∞ view loads it (the full
//! tier, like the `htr10` rung's fuel zone).

use super::model::fhr;
use crate::engine::Tier;
use crate::tapes::read_tape;
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
    pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        let i = self.done.len() + usize::from(self.sab.is_some());
        let (label, _) = *JOBS.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, label)?;
        if i < NAMES.len() {
            let n =
                Nuclide::from_tape_with_speed(&tape, mat, NAMES[i], fhr::TEMP_K, SpeedTier::Fast)
                    .map_err(|e| format!("{label}: {e}"))?;
            self.done.push(n);
        } else {
            let mat = if mat == GRAPHITE_MAT {
                mat
            } else {
                return Err(format!("{label}: MAT {mat}, expected {GRAPHITE_MAT}"));
            };
            self.sab = Some(
                ThermalScattering::from_tape(&tape, mat, fhr::TEMP_K, "c_Graphite")
                    .map_err(|e| format!("{label}: {e}"))?,
            );
        }
        Ok(())
    }

    /// The nuclide table in `fhr`'s index order, if this load processed the
    /// tapes (the full tier); `None` for the Watch view's load.
    pub fn finish(self) -> Result<Option<Vec<Nuclide>>, String> {
        if self.tier == Tier::Loose {
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
        let graphite = c12.clone().with_thermal_scattering(sab);
        let rest = [next()?, next()?, next()?, next()?, next()?];
        let mut table = vec![u235, u238, o16, c12, graphite];
        table.extend(rest);
        // The indices the material table uses (`fhr::U235` … `fhr::F19`).
        debug_assert_eq!(
            (fhr::C_FREE, fhr::C_GRAPHITE, fhr::SI28, fhr::F19),
            (3, 4, 5, 9)
        );
        Ok(Some(table))
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
            b.step(&crate::tapes::strip_covariances(&raw))
                .expect("step");
            eprintln!("  {label:<16} {:6.1} s", t.elapsed().as_secs_f64());
        }
        let n = b.finish().unwrap().unwrap();
        assert_eq!(n.len(), 10);
        // The bound law changes carbon's thermal total; the free-gas entry keeps its own.
        let at = |i: usize| n[i].total_at_energy(0.0253, fhr::TEMP_K);
        assert!(
            at(fhr::C_GRAPHITE) != at(fhr::C_FREE),
            "graphite S(a,b) not attached"
        );
    }

    /// A Watch load processes nothing and holds no table.
    #[test]
    fn a_watch_load_has_no_table() {
        assert!(DataBuilder::new(Tier::Loose).finish().unwrap().is_none());
        assert!(DataBuilder::new(Tier::Exact).finish().is_err());
    }
}
