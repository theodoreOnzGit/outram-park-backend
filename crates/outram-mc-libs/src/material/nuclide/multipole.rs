//! Windowed multipole on a **pointwise** nuclide, evaluated at the lookup
//! temperature inside its energy range (GitHub #948, #945 step 2).
//!
//! A port of how OpenMC uses multipole data in transport
//! (`src/nuclide.cpp:620-680`, OpenMC `d7d3284a1`): when a nuclide has
//! multipole data and `E_min <= E <= E_max`, its cross sections are
//! `WindowedMultipole::evaluate(E, T)` at the particle's own temperature,
//! with `total = s + a`, `elastic = s`, `absorption = a`, `fission = f`,
//! `nu_fission = f * nu(E)` and every other channel zero ("assumes that
//! inelastic scattering does not occur in the resolved resonance region",
//! `docs/source/methods/cross_sections.rst:140-142`). The URR tables are not
//! read inside the range (`nuclide.cpp:826`), S(α,β) still applies on top,
//! and the elastic kinematics take the local `kT` at every energy
//! (`src/physics.cpp:734`). Outside the range the tables answer, at their
//! build temperature.
//!
//! **One departure, on purpose:** upstream attaches whatever the
//! `cross_sections.xml` names. [`Nuclide::with_multipole`] first checks that
//! the multipole data and the nuclide's own tables are **the same
//! evaluation**, by comparing them at the tables' own temperature, and
//! refuses otherwise. The embedded CORE library reproduces ENDF/B-VII.0's
//! resolved range and is a different evaluation from the ENDF/B-VIII.0
//! tapes this workspace reads by default (U-235 capture differs by 27 % in
//! 1–2.25 keV): attaching it there would swap the evaluation, not broaden
//! it. Measured in
//! `verification_and_validation/wmp_local_temperature_948/README.md` §2.
//!
//! **Opt-in.** No constructor attaches multipole data. The reasons and the
//! measurements behind them are in the V&V record §7.

use petir::mathf::RealMath;
use super::{MicroXS, Nuclide, XsSource};
use crate::physics::scatter::K_BOLTZMANN_EV_PER_K;
use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};
use njoy_outram_park_fork::MtReaction;
use std::sync::Arc;

/// Largest difference \[-\] allowed between a multipole set and a nuclide's
/// own tables, per decade band of `∫σ dE/E` (total and absorption, each
/// relative to the band's table **total**), at the tables' temperature,
/// for [`Nuclide::with_multipole`] to accept them as one evaluation.
///
/// ~~Chosen from the 0 K probe (same evaluation ≤ 3e-5)~~ **Corrected
/// 2026-10-18**: that probe integrated both on RECONR's nodes and hid
/// RECONR's own ~1e-3 linearisation, which a broadened table carries (#948
/// V&V record §5.0). Measured at 900 K on production tables (RECONR/BROADR
/// 1e-3): the same evaluation (ENDF/B-VII.0 U-238, U-235, O-16) agrees to
/// **≤ 2.1e-4**, another evaluation (ENDF/B-VIII.0) differs by **≥ 8.6e-3**.
/// `1e-3` sits between them, ×4.7 and ×8.6 from each. A table processed
/// more coarsely than NJOY's defaults (`SpeedTier::VeryFast`, 1e-2) may be
/// refused; that is the safe failure.
pub const MULTIPOLE_EVALUATION_TOLERANCE: f64 = 1.0e-3;

/// Why [`Nuclide::with_multipole`] refused a multipole set.
#[derive(Debug, Clone, PartialEq)]
pub enum MultipoleAttachError {
    /// The nuclide is on the LOW (`Core`) tier, whose cross sections are
    /// already windowed multipole.
    NotPointwise,
    /// The nuclide's tables carry no temperature to compare at.
    NoDataTemperature,
    /// The atomic weight ratios differ by more than 1e-3 relative: not the
    /// same nuclide.
    AwrMismatch {
        /// The nuclide's own AWR.
        nuclide: f64,
        /// The multipole set's AWR.
        multipole: f64,
    },
    /// The embedded CORE library has no set under this nuclide's name.
    NotInCoreLibrary(String),
    /// The multipole set is not the evaluation the tables were made from.
    EvaluationMismatch(MultipoleConsistency),
}

impl std::fmt::Display for MultipoleAttachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPointwise => write!(f, "the LOW (Core) tier is already windowed multipole"),
            Self::NoDataTemperature => write!(f, "the nuclide's tables carry no temperature"),
            Self::AwrMismatch { nuclide, multipole } => {
                write!(f, "AWR {nuclide} (tables) against {multipole} (multipole)")
            }
            Self::NotInCoreLibrary(n) => write!(f, "{n} is not in the embedded CORE WMP library"),
            Self::EvaluationMismatch(c) => write!(
                f,
                "multipole data are another evaluation: {} band integral over \
                 [{:.3e}, {:.3e}] eV differs by {:+.3e} (tolerance {:.0e})",
                c.channel, c.e_lo, c.e_hi, c.worst_relative, MULTIPOLE_EVALUATION_TOLERANCE
            ),
        }
    }
}

impl std::error::Error for MultipoleAttachError {}

/// How far a multipole set is from a nuclide's own tables, at the tables'
/// temperature: the worst decade band of `∫σ dE/E` over the multipole
/// range, total and absorption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultipoleConsistency {
    /// `(∫σ_wmp − ∫σ_table) / ∫σ_t,table` in the worst band \[-\].
    pub worst_relative: f64,
    /// `"total"` or `"absorption"`.
    pub channel: &'static str,
    /// Lower edge of the worst band \[eV\].
    pub e_lo: f64,
    /// Upper edge of the worst band \[eV\].
    pub e_hi: f64,
    /// Table points compared.
    pub points: usize,
}

impl Nuclide {
    /// **Attach windowed-multipole data**, so the resolved range is evaluated
    /// at the lookup (material) temperature inside `[e_min, e_max]`, as
    /// OpenMC does with `temperature_multipole` on (GitHub #948). See the
    /// module doc for exactly what changes.
    ///
    /// # Errors
    ///
    /// [`MultipoleAttachError`] when the nuclide is on the LOW tier, has no
    /// data temperature, its AWR differs from the set's, or the set is
    /// **another evaluation** ([`Self::multipole_consistency`] above
    /// [`MULTIPOLE_EVALUATION_TOLERANCE`]).
    ///
    /// # Cost
    ///
    /// The check evaluates the multipole form at every table point of MT=1
    /// inside its range, once: 0.13–0.17 s for U-238 or U-235 on production
    /// tables (#948 V&V record §5.0, i9-13900K, one thread).
    pub fn with_multipole(mut self, wmp: WindowedMultipole) -> Result<Self, MultipoleAttachError> {
        let c = self.multipole_consistency(&wmp)?;
        if c.worst_relative.abs() > MULTIPOLE_EVALUATION_TOLERANCE {
            return Err(MultipoleAttachError::EvaluationMismatch(c));
        }
        self.multipole = Some(Arc::new(wmp));
        Ok(self)
    }

    /// [`Self::with_multipole`] with this nuclide's set from the embedded
    /// CORE library ([`WmpLibrary::core`], looked up by [`Nuclide::name`]).
    ///
    /// # Errors
    ///
    /// As [`Self::with_multipole`], or
    /// [`MultipoleAttachError::NotInCoreLibrary`].
    pub fn with_core_multipole(self) -> Result<Self, MultipoleAttachError> {
        let wmp = WmpLibrary::core()
            .get(&self.name)
            .map_err(|_| MultipoleAttachError::NotInCoreLibrary(self.name.clone()))?;
        self.with_multipole(wmp)
    }

    /// This nuclide **without** its multipole data: the tables at their build
    /// temperature everywhere. The ablation arm of [`Self::with_multipole`].
    pub fn without_multipole(mut self) -> Self {
        self.multipole = None;
        self
    }

    /// The attached multipole set, if any (always `None` on the LOW tier,
    /// whose multipole data is its cross-section source).
    pub fn multipole(&self) -> Option<&WindowedMultipole> {
        self.multipole.as_deref()
    }

    /// Compare `wmp` with this nuclide's own tables at the tables'
    /// temperature: per decade band of `[e_min, e_max]` (the last band ends
    /// at `e_max`), `∫σ dE/E` of the total and of the absorption by the
    /// trapezoid rule in lethargy on the MT=1 table points. Returns the worst
    /// band.
    ///
    /// # Errors
    ///
    /// [`MultipoleAttachError::NotPointwise`],
    /// [`MultipoleAttachError::NoDataTemperature`] or
    /// [`MultipoleAttachError::AwrMismatch`].
    pub fn multipole_consistency(
        &self,
        wmp: &WindowedMultipole,
    ) -> Result<MultipoleConsistency, MultipoleAttachError> {
        let XsSource::Pointwise { recon, .. } = &self.xs else {
            return Err(MultipoleAttachError::NotPointwise);
        };
        let kt = self
            .data_kt_ev
            .ok_or(MultipoleAttachError::NoDataTemperature)?;
        if (wmp.awr / self.awr - 1.0).abs() > 1.0e-3 {
            return Err(MultipoleAttachError::AwrMismatch {
                nuclide: self.awr,
                multipole: wmp.awr,
            });
        }
        let t_k = kt / K_BOLTZMANN_EV_PER_K;
        let grid: Vec<f64> = recon
            .sections
            .iter()
            .find(|s| s.mt == MtReaction::Mt1Total)
            .map(|s| s.pairs.iter().map(|p| p.0).collect())
            .unwrap_or_default();
        let grid: Vec<f64> = grid
            .into_iter()
            .filter(|&e| e >= wmp.e_min && e <= wmp.e_max && e > 0.0)
            .collect();
        // Per point: (table total, table absorption, wmp total, wmp absorption).
        let rows: Vec<(f64, [f64; 4])> = grid
            .iter()
            .map(|&e| {
                let t = self.table_xs_at_energy(e, t_k);
                let w = wmp.evaluate(e, t_k);
                (e, [t.total, t.absorption, w.total(), w.absorption])
            })
            .collect();
        let mut worst = MultipoleConsistency {
            worst_relative: 0.0,
            channel: "total",
            e_lo: wmp.e_min,
            e_hi: wmp.e_max,
            points: rows.len(),
        };
        let mut lo = wmp.e_min;
        while lo < wmp.e_max {
            let hi = (lo * 10.0).min(wmp.e_max);
            let mut acc = [0.0_f64; 4];
            let mut prev: Option<(f64, [f64; 4])> = None;
            for &(e, v) in rows.iter().filter(|(e, _)| *e >= lo && *e <= hi) {
                if let Some((e0, v0)) = prev {
                    let du = (e / e0).r_ln();
                    for k in 0..4 {
                        acc[k] += 0.5 * (v0[k] + v[k]) * du;
                    }
                }
                prev = Some((e, v));
            }
            for (k, name) in [(0, "total"), (1, "absorption")] {
                // Both channels are measured against the band's table TOTAL:
                // what transport sees is the absorption *fraction*, and a
                // relative error on an absorption of microbarns (O-16) says
                // nothing about it.
                if acc[0] > 0.0 {
                    let rel = (acc[k + 2] - acc[k]) / acc[0];
                    if rel.abs() > worst.worst_relative.abs() {
                        worst.worst_relative = rel;
                        worst.channel = name;
                        worst.e_lo = lo;
                        worst.e_hi = hi;
                    }
                }
            }
            lo = hi;
        }
        Ok(worst)
    }

    /// The attached multipole set when `e` \[eV\] is inside its range
    /// (`e_min <= e <= e_max`, both ends included as upstream).
    pub(super) fn multipole_at(&self, e: f64) -> Option<&WindowedMultipole> {
        self.multipole
            .as_deref()
            .filter(|w| e >= w.e_min && e <= w.e_max)
    }

    /// Upstream's multipole `MicroXS` (`src/nuclide.cpp:630-638`).
    pub(super) fn multipole_micro(&self, w: &WindowedMultipole, e: f64, temp_k: f64) -> MicroXS {
        let x = w.evaluate(e, temp_k);
        MicroXS {
            total: x.total(),
            elastic: x.scatter,
            fission: x.fission,
            absorption: x.absorption,
            inelastic: 0.0,
            n2n: 0.0,
            n3n: 0.0,
            mt5: 0.0,
            other: 0.0,
            nu_fission: x.fission * self.nu_bar(e),
        }
    }
}

/// Every energy \[eV\] where the multipole form changes shape: the range
/// ends (the seam with the tables), the window edges (uniform in `√E`) and
/// each pole's peak `Re(p)²`. For [`Nuclide::majorant_breakpoints`], so a
/// delta-tracking majorant puts a node at each (GitHub #948; the LOW tier's
/// pole nodes came with #585).
pub(super) fn structural_nodes(w: &WindowedMultipole) -> Vec<f64> {
    let mut v = vec![w.e_min, w.e_max];
    if w.inv_spacing > 0.0 {
        let s0 = w.e_min.sqrt();
        v.extend((0..=w.windows.len()).map(|i| {
            let s = s0 + i as f64 / w.inv_spacing;
            s * s
        }));
    }
    v.extend(w.poles.iter().filter(|p| p.re > 0.0).map(|p| p.re * p.re));
    v.retain(|&e| e >= w.e_min && e <= w.e_max);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::reference_data::reference_endf;

    /// The URR gate (upstream `src/nuclide.cpp:826`, `&& !use_mp`). The CORE
    /// data never overlap a URR (U-238's WMP ends at 19999.99 eV, its URR
    /// starts at 20 keV), so the overlap is made here by widening a copy of
    /// the set past 20 keV and attaching it unchecked, which only this module
    /// can do. Inside the widened range no band is drawn and the band lookup
    /// is the plain one; above it the tables draw as before.
    #[test]
    fn no_urr_band_inside_a_multipole_range() {
        let Some(path) = reference_endf("n-092_U_238-ENDF7.0.endf") else {
            println!("[skip] U-238 VII.0 absent");
            return;
        };
        let tape = Tape::read_file(&path).expect("tape");
        let mat = tape.materials()[0];
        let n = Nuclide::from_tape(&tape, mat, "U238", 900.0, 1.0e-3).expect("nuclide");
        assert!(
            n.has_urr_probability_tables(),
            "the ordinary path builds URR tables"
        );
        let mut w = WmpLibrary::core().get("U238").expect("U238");
        w.e_max = 3.0e4;
        let mut mp = n.clone();
        mp.multipole = Some(Arc::new(w));
        let e = 2.5e4;
        assert!(n.needs_urr_draw(e));
        assert!(!mp.needs_urr_draw(e));
        let plain = mp.xs_at_energy(e, 900.0);
        assert_eq!(
            mp.xs_at_energy_urr(e, 900.0, 0.3).total.to_bits(),
            plain.total.to_bits()
        );
        assert_eq!(
            mp.total_upper_bound(e, 900.0).to_bits(),
            plain.total.to_bits()
        );
        assert!(mp.needs_urr_draw(4.0e4));
        // The structural nodes cover the range ends, inside the range only.
        let nodes = structural_nodes(mp.multipole().expect("attached"));
        assert!(nodes.contains(&3.0e4) && nodes.iter().all(|&x| x <= 3.0e4));
    }
}
