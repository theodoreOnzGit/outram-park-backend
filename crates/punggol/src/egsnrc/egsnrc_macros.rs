// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/egsnrc.macros
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada (and the EGS4 SLAC
//                     authors for the parts of the file that carry that notice)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! Hand expansion of the `egsnrc.macros` pieces the first ported routines use
//! (gh:#1680).
//!
//! **Status: first port, not differentially tested (gh:#1680).**
//!
//! | Mortran | here |
//! |---|---|
//! | `COMIN/STACK/` (`egsnrc.macros:1064-1068`) | [`Stack`] (owned, passed explicitly; no global state) |
//! | `$TRANSFERPROPERTIESTO#FROM#;` (`:1271-1273`) | [`Stack::transfer_properties`] |
//! | `$CHECK-STACK(#,#);` (`:1281-1288`) | [`Stack::check_stack`] (returns an error instead of `$egs_fatal`) |
//! | `$SELECT-AZIMUTHAL-ANGLE(#,#);` (`:2491-2507`) | [`select_azimuthal_angle`] |
//! | `$RANDOMSET` | [`UniformSource`], implemented for PETIR's RANMAR and RANLUX |
//!
//! Stack indices are **1-based as in Mortran** (index 0 is unused) so the ported
//! routines read line for line against upstream.
//!
//! Not expanded yet: everything else in the 3780-line macro file (the physics
//! switches, `$AUSCALL` scoring hooks, option defaults).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use alloc::vec::Vec;
use petir::rng::ranlux::Ranlux;
use petir::rng::ranmar::Ranmar;

/// `$RANDOMSET`: a uniform deviate on (0, 1).
pub trait UniformSource {
    /// The next uniform deviate.
    fn uniform(&mut self) -> f64;
}

impl UniformSource for Ranmar {
    fn uniform(&mut self) -> f64 {
        self.get_uniform()
    }
}

impl UniformSource for Ranlux {
    fn uniform(&mut self) -> f64 {
        self.get_uniform()
    }
}

/// An error where upstream calls `$egs_fatal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgsError {
    /// `$CHECK-STACK` failed: `needed` slots in a stack of `mxstack`, in `routine`.
    StackExceeded {
        /// The subroutine that asked.
        routine: &'static str,
        /// The index it needs.
        needed: usize,
        /// `$MXSTACK`.
        mxstack: usize,
    },
    /// `$egs_fatal`: upstream stops the run (named in the message).
    Fatal(&'static str),
    /// A branch that is not ported was requested (named in the message).
    Unsupported(&'static str),
    /// `UPHI` was called with an `ientry` or `lvl` outside 1..=3.
    BadEntry {
        /// `IENTRY`.
        ientry: i32,
        /// `LVL`.
        lvl: i32,
    },
}

/// `COMIN/STACK/`: the particle stack, structure-of-arrays, 1-based.
#[derive(Debug, Clone)]
pub struct Stack {
    /// Total energy, MeV.
    pub e: Vec<f64>,
    /// Position x, cm.
    pub x: Vec<f64>,
    /// Position y.
    pub y: Vec<f64>,
    /// Position z.
    pub z: Vec<f64>,
    /// Direction cosine u.
    pub u: Vec<f64>,
    /// Direction cosine v.
    pub v: Vec<f64>,
    /// Direction cosine w.
    pub w: Vec<f64>,
    /// Distance to the nearest boundary.
    pub dnear: Vec<f64>,
    /// Statistical weight.
    pub wt: Vec<f64>,
    /// Charge: -1, 0, +1.
    pub iq: Vec<i32>,
    /// Region.
    pub ir: Vec<i32>,
    /// User latch.
    pub latch: Vec<i32>,
    /// `LATCHI`.
    pub latchi: i32,
    /// `NP`, the top of the stack.
    pub np: usize,
    /// `NPold`.
    pub np_old: usize,
    /// The `save`d variables of `sample_triplet` (`egsnrc.mortran:4744-4757`):
    /// Fortran keeps them between calls and histories, so they travel with the
    /// stack, which a caller reuses across histories to keep them.
    pub triplet: super::egsnrc_mortran::TripletState,
    /// `x_final, y_final, z_final` (a `COMMON` of `egsnrc.macros:911`): the
    /// position at the end of the last `PHOTON` or `ELECTR` step. It persists
    /// across calls and histories; `$ADD_WORK_EM_FIELD` of `EEMF_macros` reads
    /// it before `ELECTR` assigns it for the step.
    pub x_final: [f64; 3],
    mxstack: usize,
}

impl Stack {
    /// An empty stack of capacity `mxstack` (`$MXSTACK`).
    pub fn new(mxstack: usize) -> Self {
        let n = mxstack + 1;
        Self {
            e: alloc::vec![0.0; n],
            x: alloc::vec![0.0; n],
            y: alloc::vec![0.0; n],
            z: alloc::vec![0.0; n],
            u: alloc::vec![0.0; n],
            v: alloc::vec![0.0; n],
            w: alloc::vec![0.0; n],
            dnear: alloc::vec![0.0; n],
            wt: alloc::vec![0.0; n],
            iq: alloc::vec![0; n],
            ir: alloc::vec![0; n],
            latch: alloc::vec![0; n],
            latchi: 0,
            np: 0,
            np_old: 0,
            triplet: super::egsnrc_mortran::TripletState::default(),
            x_final: [0.0; 3],
            mxstack,
        }
    }

    /// `$MXSTACK`.
    pub fn mxstack(&self) -> usize {
        self.mxstack
    }

    /// `$TRANSFER PROPERTIES TO (to) FROM (from)`: copies `x y z ir wt dnear
    /// latch` (not `e`, `iq`, `u`, `v`, `w`).
    pub fn transfer_properties(&mut self, to: usize, from: usize) {
        self.x[to] = self.x[from];
        self.y[to] = self.y[from];
        self.z[to] = self.z[from];
        self.ir[to] = self.ir[from];
        self.wt[to] = self.wt[from];
        self.dnear[to] = self.dnear[from];
        self.latch[to] = self.latch[from];
    }

    /// `$CHECK-STACK(needed, routine)`.
    ///
    /// # Errors
    ///
    /// [`EgsError::StackExceeded`] if `needed > $MXSTACK`.
    pub fn check_stack(&self, needed: usize, routine: &'static str) -> Result<(), EgsError> {
        if needed > self.mxstack {
            return Err(EgsError::StackExceeded { routine, needed, mxstack: self.mxstack });
        }
        Ok(())
    }
}

/// `$SELECT-AZIMUTHAL-ANGLE(cphi, sphi)` (`egsnrc.macros:2491-2507`): a point
/// in the upper half-disc by rejection, then the double-angle formulas.
/// Returns `(cos phi, sin phi)`. Uses the reciprocal of `rho^2` as the macro
/// does (not a division), so the roundoff matches upstream's order.
pub fn select_azimuthal_angle<R: UniformSource>(rng: &mut R) -> (f64, f64) {
    let mut xphi;
    let mut xphi2;
    let mut yphi2;
    let mut yphi;
    let mut rhophi2;
    loop {
        xphi = rng.uniform();
        xphi = 2.0 * xphi - 1.0;
        xphi2 = xphi * xphi;
        yphi = rng.uniform();
        yphi2 = yphi * yphi;
        rhophi2 = xphi2 + yphi2;
        if !(rhophi2 > 1.0) {
            break;
        }
    }
    rhophi2 = 1.0 / rhophi2;
    ((xphi2 - yphi2) * rhophi2, 2.0 * xphi * yphi * rhophi2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azimuth_is_a_unit_vector_and_isotropic() {
        let mut r = Ranmar::new(1802, 9373);
        let (mut sc, mut ss) = (0.0, 0.0);
        let n = 4000;
        for _ in 0..n {
            let (c, s) = select_azimuthal_angle(&mut r);
            assert!((c * c + s * s - 1.0).abs() < 1e-12);
            sc += c;
            ss += s;
        }
        assert!((sc / n as f64).abs() < 0.05 && (ss / n as f64).abs() < 0.05);
        let mut l = Ranlux::new(1, 12345);
        let (c, s) = select_azimuthal_angle(&mut l);
        assert!((c * c + s * s - 1.0).abs() < 1e-12);
    }

    #[test]
    fn stack_transfer_and_check() {
        let mut st = Stack::new(5);
        st.x[1] = 1.0;
        st.y[1] = 2.0;
        st.z[1] = 3.0;
        st.ir[1] = 7;
        st.wt[1] = 0.5;
        st.dnear[1] = 4.0;
        st.latch[1] = 9;
        st.e[1] = 11.0;
        st.transfer_properties(2, 1);
        assert_eq!((st.x[2], st.y[2], st.z[2], st.ir[2], st.wt[2], st.dnear[2], st.latch[2]), (1.0, 2.0, 3.0, 7, 0.5, 4.0, 9));
        assert_eq!(st.e[2], 0.0, "energy is not a transferred property");
        assert!(st.check_stack(5, "T").is_ok());
        assert_eq!(st.check_stack(6, "T"), Err(EgsError::StackExceeded { routine: "T", needed: 6, mxstack: 5 }));
        assert_eq!(st.mxstack(), 5);
    }
}
