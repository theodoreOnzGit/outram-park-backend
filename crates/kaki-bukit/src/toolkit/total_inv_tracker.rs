// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/total_inv_tracker.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! A facility-wide inventory cap spanning several [`ResBuf`]s.
//!
//! A facility that keeps its stock in more than one buffer — an inventory and
//! an outventory, say — often has a licence condition or a site limit on the
//! **total**, independent of what each buffer can hold. [`TotalInvTracker`] is
//! that limit. It stores no material itself; it only answers questions about
//! buffers it is shown.
//!
//! All masses are **kilograms**.
//!
//! # Divergence: the buffers are passed in, not held
//!
//! Upstream holds `std::vector<ResBuf<Material>*>` — raw pointers into buffers
//! that live on the agent — and every query walks that vector. This workspace
//! forbids lifetime parameters on structs, so the tracker cannot hold
//! borrows, and holding `Arc<RwLock<ResBuf>>` would force every facility to
//! wrap its inventories in a lock they otherwise do not need.
//!
//! So the buffers travel as an argument: `tracker.quantity(&[&inv, &outv])`.
//! The consequences are worth stating plainly, because they are the whole
//! difference:
//!
//! - **The caller is responsible for passing the same set every time.** A
//!   facility that forgets a buffer under-reports its own inventory. Upstream's
//!   `Init` fixes the set once; here the discipline is at the call site, and a
//!   facility should have exactly one private method that builds the slice.
//! - Upstream's `buf_in_tracker` is meaningless here and is not ported —
//!   membership is decided by what the caller passes.
//! - Upstream throws if the tracker was never initialised (`num_bufs() == 0`).
//!   The equivalent here is an empty slice, and it is refused the same way; see
//!   [`CyclusError::State`].
//!
//! [`ResBuf`]: crate::toolkit::res_buf::ResBuf
//! [`CyclusError::State`]: crate::error::CyclusError::State

use crate::error::{CyclusError, Result};
use crate::limits::EPS_RSRC;
use crate::toolkit::res_buf::ResBuf;

/// A cap, in kilograms, on the combined contents of several [`ResBuf`]s.
/// Upstream `TotalInvTracker`.
///
/// Holds no material. Every query takes the buffers it should look at — see
/// the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TotalInvTracker {
    max_inv_size: f64,
}

impl Default for TotalInvTracker {
    /// An effectively unbounded tracker. Upstream's default-constructed
    /// `TotalInvTracker` uses `std::numeric_limits<double>::max()`, and so does
    /// this.
    fn default() -> Self {
        Self {
            max_inv_size: f64::MAX,
        }
    }
}

impl TotalInvTracker {
    /// An unbounded tracker. Upstream `TotalInvTracker()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A tracker capping the combined inventory at `max_inv_size` kilograms.
    /// Upstream `TotalInvTracker(bufs, max_inv_size)` / `Init`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `max_inv_size` is not strictly positive, or
    /// is NaN. Upstream rejects `<= 0` for the same reason: a zero-capacity
    /// facility can never accept anything, which is always a configuration
    /// mistake rather than an intent.
    ///
    /// [`CyclusError::Value`]: crate::error::CyclusError::Value
    pub fn with_max(max_inv_size: f64) -> Result<Self> {
        if max_inv_size.is_nan() || max_inv_size <= 0.0 {
            return Err(CyclusError::Value(
                "total inventory tracker capacity must be positive",
            ));
        }
        Ok(Self { max_inv_size })
    }

    /// The tracker's own cap, in kilograms, ignoring the buffers. Upstream
    /// `tracker_capacity()`.
    #[must_use]
    pub fn tracker_capacity(&self) -> f64 {
        self.max_inv_size
    }

    /// The combined mass held by `bufs`, in kilograms. Upstream `quantity()`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::State`](crate::error::CyclusError::State) if `bufs` is
    /// empty — the equivalent of upstream's uninitialised tracker.
    pub fn quantity(&self, bufs: &[&ResBuf]) -> Result<f64> {
        check(bufs)?;
        Ok(bufs.iter().map(|b| b.quantity()).sum())
    }

    /// The sum of the buffers' own capacities, in kilograms, ignoring the
    /// tracker's cap. Upstream `total_capacity_bufs()`.
    ///
    /// # Errors
    ///
    /// As [`quantity`](TotalInvTracker::quantity).
    pub fn total_capacity_bufs(&self, bufs: &[&ResBuf]) -> Result<f64> {
        check(bufs)?;
        Ok(bufs.iter().map(|b| b.capacity()).sum())
    }

    /// The effective capacity, in kilograms: the lesser of the tracker's cap
    /// and the sum of the buffers' capacities. Upstream `capacity()`.
    ///
    /// # Errors
    ///
    /// As [`quantity`](TotalInvTracker::quantity).
    pub fn capacity(&self, bufs: &[&ResBuf]) -> Result<f64> {
        let bufs_cap = self.total_capacity_bufs(bufs)?;
        Ok(if bufs_cap < self.max_inv_size {
            bufs_cap
        } else {
            self.max_inv_size
        })
    }

    /// The mass that may still be accepted across all buffers, in kilograms,
    /// never negative. Upstream `space()`.
    ///
    /// # Errors
    ///
    /// As [`quantity`](TotalInvTracker::quantity).
    pub fn space(&self, bufs: &[&ResBuf]) -> Result<f64> {
        let s = self.capacity(bufs)? - self.quantity(bufs)?;
        Ok(if s > 0.0 { s } else { 0.0 })
    }

    /// How much one buffer may still accept once the facility-wide limit is
    /// taken into account, in kilograms. Upstream `constrained_buf_space()`.
    ///
    /// This is the number a buy policy should request against — a buffer with
    /// 400 kg of its own space in a facility with 50 kg of site-wide space can
    /// accept 50 kg.
    ///
    /// # Errors
    ///
    /// As [`quantity`](TotalInvTracker::quantity).
    pub fn constrained_buf_space(&self, bufs: &[&ResBuf], buf: &ResBuf) -> Result<f64> {
        let site = self.space(bufs)?;
        let own = buf.space();
        Ok(if own < site { own } else { site })
    }

    /// `true` if every tracked buffer is empty. Upstream `empty()`.
    ///
    /// # Errors
    ///
    /// As [`quantity`](TotalInvTracker::quantity).
    pub fn is_empty(&self, bufs: &[&ResBuf]) -> Result<bool> {
        Ok(self.quantity(bufs)? == 0.0)
    }

    /// Changes the facility-wide cap, in kilograms. Upstream `set_capacity()`.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`](crate::error::CyclusError::Value) if `cap` is
    ///   below the mass already held, by more than
    ///   [`EPS_RSRC`](crate::limits::EPS_RSRC) — the same tolerance
    ///   [`ResBuf::set_capacity`] uses, and for the same reason.
    /// - As [`quantity`](TotalInvTracker::quantity) if `bufs` is empty.
    pub fn set_capacity(&mut self, cap: f64, bufs: &[&ResBuf]) -> Result<()> {
        if self.quantity(bufs)? - cap > EPS_RSRC {
            return Err(CyclusError::Value(
                "new total capacity is lower than the quantity already held",
            ));
        }
        self.max_inv_size = cap;
        Ok(())
    }
}

/// Upstream `num_bufs()`'s guard: an empty set means the tracker was never
/// given anything to track.
fn check(bufs: &[&ResBuf]) -> Result<()> {
    if bufs.is_empty() {
        Err(CyclusError::State(
            "total inventory tracker has no buffers to track",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::composition::{AtomicMasses, Composition};
    use crate::limits::abs;
    use crate::material::Material;
    use crate::nuclide::nuc;
    use crate::resource::Resource;

    fn mat(qty: f64) -> Resource {
        let map: CompMap = [(nuc::U238, 1.0)].into_iter().collect();
        let c = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        Resource::from(Material::new(qty, c).unwrap())
    }

    #[test]
    fn sums_the_quantities_of_every_tracked_buffer() {
        let mut inv = ResBuf::with_capacity(500.0).unwrap();
        let mut outv = ResBuf::with_capacity(500.0).unwrap();
        inv.push(mat(120.0)).unwrap();
        outv.push(mat(80.0)).unwrap();

        let t = TotalInvTracker::with_max(1000.0).unwrap();
        assert!(abs(t.quantity(&[&inv, &outv]).unwrap() - 200.0) < 1e-12);
        assert!(!t.is_empty(&[&inv, &outv]).unwrap());
    }

    #[test]
    fn capacity_is_the_lesser_of_the_tracker_and_the_buffers() {
        let inv = ResBuf::with_capacity(500.0).unwrap();
        let outv = ResBuf::with_capacity(500.0).unwrap();

        // Tracker tighter than the buffers.
        let tight = TotalInvTracker::with_max(300.0).unwrap();
        assert_eq!(tight.capacity(&[&inv, &outv]).unwrap(), 300.0);

        // Buffers tighter than the tracker.
        let loose = TotalInvTracker::with_max(5000.0).unwrap();
        assert_eq!(loose.capacity(&[&inv, &outv]).unwrap(), 1000.0);
        assert_eq!(loose.total_capacity_bufs(&[&inv, &outv]).unwrap(), 1000.0);
    }

    #[test]
    fn constrained_space_reflects_the_site_wide_limit() {
        let mut inv = ResBuf::with_capacity(500.0).unwrap();
        let mut outv = ResBuf::with_capacity(500.0).unwrap();
        inv.push(mat(100.0)).unwrap();
        outv.push(mat(150.0)).unwrap();

        // Site cap 300 kg, 250 kg already held -> 50 kg of site-wide space,
        // even though the inventory buffer itself has 400 kg free.
        let t = TotalInvTracker::with_max(300.0).unwrap();
        assert!(abs(inv.space() - 400.0) < 1e-12);
        assert!(abs(t.space(&[&inv, &outv]).unwrap() - 50.0) < 1e-12);
        assert!(abs(t.constrained_buf_space(&[&inv, &outv], &inv).unwrap() - 50.0) < 1e-12);

        // When the buffer is the binding constraint, the buffer wins.
        let loose = TotalInvTracker::with_max(100_000.0).unwrap();
        assert!(abs(loose.constrained_buf_space(&[&inv, &outv], &inv).unwrap() - 400.0) < 1e-12);
    }

    #[test]
    fn space_never_goes_negative() {
        let mut inv = ResBuf::with_capacity(500.0).unwrap();
        inv.push(mat(400.0)).unwrap();
        let t = TotalInvTracker::with_max(100.0).unwrap();
        assert_eq!(t.space(&[&inv]).unwrap(), 0.0);
    }

    #[test]
    fn an_empty_buffer_set_is_refused() {
        let t = TotalInvTracker::new();
        assert!(t.quantity(&[]).is_err());
        assert!(t.capacity(&[]).is_err());
        assert!(t.space(&[]).is_err());
    }

    #[test]
    fn a_non_positive_cap_is_refused() {
        assert!(TotalInvTracker::with_max(0.0).is_err());
        assert!(TotalInvTracker::with_max(-1.0).is_err());
        assert!(TotalInvTracker::with_max(f64::NAN).is_err());
    }

    #[test]
    fn the_cap_cannot_be_lowered_below_the_contents() {
        let mut inv = ResBuf::with_capacity(500.0).unwrap();
        inv.push(mat(300.0)).unwrap();
        let mut t = TotalInvTracker::with_max(500.0).unwrap();
        assert!(t.set_capacity(100.0, &[&inv]).is_err());
        assert_eq!(t.tracker_capacity(), 500.0);
        t.set_capacity(400.0, &[&inv]).unwrap();
        assert_eq!(t.tracker_capacity(), 400.0);
    }
}
