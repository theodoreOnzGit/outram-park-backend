// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Photon-group weighting functions for the GAMINR module.
//!
//! # What this module computes
//!
//! GAMINR group-averages each photon cross section over a **weighting spectrum**
//! `w(E)` (the assumed photon flux shape within a group). The weight option is
//! chosen by the card-2 integer `iwt`. This module ports:
//!
//! - the option selector and built-in weight data from `gnwtf`
//!   (`gaminr.f90:778-823`), and
//! - the in-range weight *evaluation* used inside the group-averaging quadrature
//!   from `gtflx` (`gaminr.f90:825-872`) for the two built-in analytic options.
//!
//! # Options (`iwt`, from `gaminr.f90:84-92, 800-821`)
//!
//! | `iwt` | meaning |
//! |------:|---------|
//! | 1 | weight read in as an ENDF TAB1 record (arbitrary) — **NotPorted** |
//! | 2 | constant weight `w(E) = 1` for all Legendre orders |
//! | 3 | 1/E with high- and low-energy roll-offs (built-in 4-point TAB1) |
//!
//! # Energy and weight units
//!
//! Energy `E` is in **electron-volts (eV)**. The weight `w(E)` is a **relative,
//! dimensionless** flux-per-unit-energy shape — only its shape matters, since
//! group averaging forms the ratio `∫ σ w dE / ∫ w dE`. The absolute scale is
//! arbitrary.
//!
//! ## Fidelity note (untrusted AI draft)
//!
//! `gtflx` evaluates the tabulated weight with NJOY's `terpa` (ENDF TAB1
//! interpolation) and applies a `step = 1.05` panel-subdivision limiter
//! (`gaminr.f90:838,858-860`) that belongs to the quadrature driver, not the
//! weight itself. This port reproduces the **in-range TAB1 interpolation**
//! (log-log / ENDF interpolation law 5 for the built-in option 3) needed to
//! evaluate `w(E)`; it does **not** reproduce `terpa`'s out-of-range /
//! discontinuity bookkeeping or the panel limiter. Evaluations outside the
//! tabulated energy range clamp to the endpoint value and are flagged in
//! [`PhotonWeight::evaluate`].

use crate::NjoyError;

/// Photon weighting-function option — the `iwt` card-2 integer
/// (`gaminr.f90:84-92, 800-821`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightOption {
    /// `iwt = 1`: weight read in from the input deck as an ENDF TAB1 record
    /// (`gaminr.f90:803-805`). No built-in shape — see [`PhotonWeight::from_option`].
    ReadIn,
    /// `iwt = 2`: constant weight for all Legendre orders (`gaminr.f90:808-809`).
    Constant,
    /// `iwt = 3`: 1/E with high- and low-energy roll-offs, the built-in
    /// `wt1` 4-point TAB1 (`gaminr.f90:795-798, 812-816`).
    OneOverEWithRolloffs,
}

impl WeightOption {
    /// Build a [`WeightOption`] from the raw `iwt` integer.
    ///
    /// Mirrors `gnwtf`'s branch (`gaminr.f90:802-821`). An `iwt` outside
    /// `1..=3` is GAMINR's "illegal iwt" abort (`gaminr.f90:819-820`); here it
    /// returns [`NjoyError::EndfParse`] rather than aborting.
    pub fn from_iwt(iwt: i32) -> Result<Self, NjoyError> {
        Ok(match iwt {
            1 => Self::ReadIn,
            2 => Self::Constant,
            3 => Self::OneOverEWithRolloffs,
            other => {
                return Err(NjoyError::EndfParse(format!(
                    "gaminr gnwtf: illegal weight option iwt={other} (valid 1..=3)"
                )));
            }
        })
    }

    /// The raw `iwt` integer for this option.
    pub fn iwt(self) -> i32 {
        match self {
            Self::ReadIn => 1,
            Self::Constant => 2,
            Self::OneOverEWithRolloffs => 3,
        }
    }
}

/// A ready-to-evaluate photon weighting spectrum `w(E)`.
///
/// Constructed from a [`WeightOption`] via [`PhotonWeight::from_option`]. The
/// read-in TAB1 option is not yet ported.
#[derive(Debug, Clone, PartialEq)]
pub enum PhotonWeight {
    /// Constant weight `w(E) = 1` (`iwt = 2`, `gaminr.f90:862-868`).
    Constant,
    /// Built-in 1/E-with-roll-offs 4-point TAB1 (`iwt = 3`), log-log
    /// interpolated (`gaminr.f90:795-798`).
    OneOverEWithRolloffs(WeightTab1),
}

impl PhotonWeight {
    /// Build the concrete weight spectrum for a [`WeightOption`].
    ///
    /// # Errors
    ///
    /// - [`WeightOption::ReadIn`] (`iwt = 1`) reads an arbitrary TAB1 from the
    ///   input deck; no built-in data exists, so this returns
    ///   [`NjoyError::NotPorted`] (`gaminr.f90:803-805`).
    pub fn from_option(opt: WeightOption) -> Result<Self, NjoyError> {
        Ok(match opt {
            WeightOption::ReadIn => {
                return Err(NjoyError::NotPorted(
                    "gaminr gnwtf iwt=1 read-in TAB1 weight (from input deck)",
                ));
            }
            WeightOption::Constant => Self::Constant,
            WeightOption::OneOverEWithRolloffs => {
                Self::OneOverEWithRolloffs(WeightTab1::one_over_e_rolloffs())
            }
        })
    }

    /// Evaluate the weight `w(E)` at photon energy `e_ev` (in **eV**).
    ///
    /// Returns the dimensionless relative weight. For
    /// [`PhotonWeight::Constant`] this is always `1.0` (`gaminr.f90:864-866`).
    /// For [`PhotonWeight::OneOverEWithRolloffs`] it is the log-log (ENDF
    /// interpolation law 5) interpolation of the built-in 4-point table.
    ///
    /// # Out-of-range behaviour
    ///
    /// Energies below the first tabulated point or above the last clamp to the
    /// corresponding endpoint weight. This is a documented simplification: NJOY's
    /// `terpa` has its own out-of-range convention that this port does not
    /// reproduce (see the module-level fidelity note).
    pub fn evaluate(&self, e_ev: f64) -> f64 {
        match self {
            Self::Constant => 1.0,
            Self::OneOverEWithRolloffs(tab) => tab.interpolate_loglog(e_ev),
        }
    }
}

/// The built-in GAMINR photon weight TAB1 tables (currently only the option-3
/// 1/E-with-roll-offs spectrum). Energies in **eV**, weights **dimensionless**.
///
/// Faithful transcription of the `wt1` DATA array (`gaminr.f90:795-798`), which
/// is laid out as an ENDF TAB1 record:
/// `C1=0, C2=0, L1=0, L2=0, NR=1, NP=4`, one interpolation region
/// `NBT(1)=4, INT(1)=5` (log-log), then four (E, w) pairs.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightTab1 {
    /// TAB1 interpolation-region boundaries `NBT` (last-point index of each
    /// region). For the built-in table this is `[4]`.
    pub nbt: Vec<usize>,
    /// TAB1 interpolation laws `INT` per region (ENDF: 5 = log-log). `[5]`.
    pub interp: Vec<i32>,
    /// Tabulated photon energies in **eV** (ascending).
    pub energies_ev: Vec<f64>,
    /// Tabulated relative weights (dimensionless), one per energy.
    pub weights: Vec<f64>,
}

impl WeightTab1 {
    /// The built-in `iwt = 3` "1/E with high and low energy roll-offs" table.
    ///
    /// Transcribed from `wt1` (`gaminr.f90:795-798`): points
    /// `(1.0e3 eV, 1.0e-4), (1.0e5 eV, 1.0), (1.0e7 eV, 1.0e-2), (3.0e7 eV, 1.0e-4)`
    /// with log-log interpolation. The rising 1e3→1e5 eV limb and the falling
    /// 1e5→3e7 eV limb give the characteristic low- and high-energy roll-offs
    /// bracketing the ~1/E plateau.
    pub fn one_over_e_rolloffs() -> Self {
        Self {
            nbt: vec![4],
            interp: vec![5],
            energies_ev: vec![1.0e3, 1.0e5, 1.0e7, 3.0e7],
            weights: vec![1.0e-4, 1.0, 1.0e-2, 1.0e-4],
        }
    }

    /// Log-log (ENDF interpolation law 5) interpolation of this table at `e_ev`.
    ///
    /// All built-in points have strictly positive energy and weight, so
    /// `ln` is well defined. Outside the tabulated range the endpoint weight is
    /// returned (see [`PhotonWeight::evaluate`] for the caveat).
    pub fn interpolate_loglog(&self, e_ev: f64) -> f64 {
        let xs = &self.energies_ev;
        let ys = &self.weights;
        debug_assert_eq!(xs.len(), ys.len());
        if e_ev <= xs[0] {
            return ys[0];
        }
        let n = xs.len();
        if e_ev >= xs[n - 1] {
            return ys[n - 1];
        }
        // Find the bracketing interval [x1, x2].
        let mut i = 0;
        while i + 1 < n && e_ev > xs[i + 1] {
            i += 1;
        }
        let (x1, x2) = (xs[i], xs[i + 1]);
        let (y1, y2) = (ys[i], ys[i + 1]);
        // ENDF law 5 (ln-ln): y = exp( ln y1 + (ln y2 - ln y1)
        //                              * (ln E - ln x1)/(ln x2 - ln x1) ).
        let t = (e_ev.ln() - x1.ln()) / (x2.ln() - x1.ln());
        (y1.ln() + (y2.ln() - y1.ln()) * t).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: `iwt` maps 1->ReadIn, 2->Constant, 3->OneOverE per
    /// `gnwtf` (`gaminr.f90:802-821`); out-of-range is an error.
    /// Result (2026-07-15): mapping and error path match the Fortran branch.
    #[test]
    fn iwt_selector_roundtrip() {
        for iwt in 1..=3 {
            assert_eq!(WeightOption::from_iwt(iwt).unwrap().iwt(), iwt);
        }
        assert!(WeightOption::from_iwt(0).is_err());
        assert!(WeightOption::from_iwt(4).is_err());
    }

    /// Methodology: iwt=1 is read-from-deck (NotPorted); iwt=2 builds a constant
    /// weight; iwt=3 builds the 4-point built-in table (`gaminr.f90:795-798`).
    /// Result (2026-07-15): constant evaluates to 1.0 at any energy; the
    /// built-in table carries the exact 4 (E,w) pairs from `wt1`.
    #[test]
    fn build_from_option() {
        assert!(matches!(
            PhotonWeight::from_option(WeightOption::ReadIn),
            Err(NjoyError::NotPorted(_))
        ));

        let c = PhotonWeight::from_option(WeightOption::Constant).unwrap();
        assert_eq!(c.evaluate(1.0), 1.0);
        assert_eq!(c.evaluate(1.0e6), 1.0);

        let w = PhotonWeight::from_option(WeightOption::OneOverEWithRolloffs).unwrap();
        if let PhotonWeight::OneOverEWithRolloffs(tab) = &w {
            assert_eq!(tab.energies_ev, vec![1.0e3, 1.0e5, 1.0e7, 3.0e7]);
            assert_eq!(tab.weights, vec![1.0e-4, 1.0, 1.0e-2, 1.0e-4]);
            assert_eq!(tab.nbt, vec![4]);
            assert_eq!(tab.interp, vec![5]);
        } else {
            panic!("expected OneOverEWithRolloffs");
        }
    }

    /// Methodology: log-log interpolation must reproduce the tabulated weight
    /// exactly at each node, clamp outside the range, and return the geometric
    /// mean at the geometric-mean energy of an interval (a property of ln-ln
    /// interpolation).
    ///
    /// Results (2026-07-15, built-in `wt1` table):
    /// - w(1e3)=1e-4, w(1e5)=1.0, w(1e7)=1e-2, w(3e7)=1e-4 (nodes, exact).
    /// - w(1e2)=1e-4 and w(1e8)=1e-4 (clamped low/high).
    /// - at E=sqrt(1e3*1e5)=1e4 eV, w=sqrt(1e-4*1.0)=1e-2 (ln-ln midpoint).
    #[test]
    fn loglog_interpolation() {
        let tab = WeightTab1::one_over_e_rolloffs();
        // Nodes exact.
        assert!((tab.interpolate_loglog(1.0e3) - 1.0e-4).abs() < 1e-12);
        assert!((tab.interpolate_loglog(1.0e5) - 1.0).abs() < 1e-12);
        assert!((tab.interpolate_loglog(1.0e7) - 1.0e-2).abs() < 1e-12);
        assert!((tab.interpolate_loglog(3.0e7) - 1.0e-4).abs() < 1e-12);
        // Clamped outside range.
        assert_eq!(tab.interpolate_loglog(1.0e2), 1.0e-4);
        assert_eq!(tab.interpolate_loglog(1.0e8), 1.0e-4);
        // ln-ln midpoint: geometric mean energy -> geometric mean weight.
        let mid = tab.interpolate_loglog((1.0e3f64 * 1.0e5).sqrt());
        assert!((mid - 1.0e-2).abs() < 1e-9, "got {mid}");
    }
}
