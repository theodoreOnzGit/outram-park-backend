// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/res_buf.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! The inventory buffer every facility holds its stock in.
//!
//! [`ResBuf`] is a FIFO queue of [`Resource`]s with a capacity. A facility
//! declares one per stream — an inventory and an outventory, say — fills them
//! through the resource exchange, and moves material between them on each time
//! step. It is the single most-used type in the whole toolkit.
//!
//! ```
//! use outram_park_fork_cyclus::comp_math::CompMap;
//! use outram_park_fork_cyclus::composition::{AtomicMasses, Composition};
//! use outram_park_fork_cyclus::material::Material;
//! use outram_park_fork_cyclus::nuclide::nuc;
//! use outram_park_fork_cyclus::resource::Resource;
//! use outram_park_fork_cyclus::toolkit::res_buf::ResBuf;
//!
//! let map: CompMap = [(nuc::U238, 1.0)].into_iter().collect();
//! let comp = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
//!
//! let mut inventory = ResBuf::with_capacity(1000.0).unwrap();  // kilograms
//! inventory.push(Resource::from(Material::new(600.0, comp.clone()).unwrap())).unwrap();
//! assert_eq!(inventory.space(), 400.0);
//!
//! // Take a 250 kg batch; the 600 kg object is split, and 350 kg stays.
//! let batch = inventory.pop_qty(250.0).unwrap();
//! assert_eq!(batch.len(), 1);
//! assert_eq!(inventory.quantity(), 350.0);
//! ```
//!
//! # Units
//!
//! Every quantity here — [`capacity`](ResBuf::capacity),
//! [`quantity`](ResBuf::quantity), [`space`](ResBuf::space), and the `qty`
//! argument to [`pop_qty`](ResBuf::pop_qty) — is a **mass in kilograms**, the
//! unit [`Material::units`](crate::material::Material::units) reports.
//! [`count`](ResBuf::count) is a dimensionless object count.
//!
//! # Why this is not generic
//!
//! Upstream is `template <class T> class ResBuf`, instantiated at
//! `ResBuf<Material>` and `ResBuf<Product>`, and every `Push` does a
//! `dynamic_pointer_cast<T>` that throws `CastError` when an agent pushes the
//! wrong kind of resource into a buffer. **This port holds
//! [`Resource`](crate::resource::Resource) — the crate's non-generic owned
//! enum — instead**, for two reasons:
//!
//! 1. A generic `ResBuf<T>` would have to be bounded by a resource trait, and
//!    the workspace forbids trait objects; the enum *is* how this crate does
//!    closed-set polymorphism (see the crate root's translation table).
//! 2. Upstream's run-time `CastError` becomes a [`Resource`] variant that the
//!    consumer matches on. Nothing is lost — a facility that wants materials
//!    only calls [`Resource::into_material`](crate::resource::Resource::into_material),
//!    which reports the same mistake, at the same moment, with a better message.
//!
//! # What is deliberately not here
//!
//! | Upstream | Status |
//! |---|---|
//! | `rs_present_`, the duplicate-push guard | **Unrepresentable, so removed.** Upstream stores `shared_ptr`s and two buffers can hold the same object; here a [`Resource`] is owned by exactly one place and a duplicate push cannot be written. |
//! | `is_bulk_` / bulk squashing on push | Squashing needs `Material::Absorb`, which needs an [`AtomicMasses`](crate::composition::AtomicMasses) table — upstream reaches one globally through PyNE, this crate has no nuclear data of its own. Call [`squash`](ResBuf::squash) explicitly instead; it does the same thing at a moment the caller chooses, with the table named. |
//! | `keep_packaging_` / `ChangePackage` | There is no `Package` type in this kernel. |
//! | `Pop(qty)` returning one squashed resource | Same reason as bulk mode: upstream's `res_manip::Squash`. [`pop_qty`](ResBuf::pop_qty) returns the pieces; squash them with [`squash_all`] if you want one object. |
//! | `Decay(curr_time)` | [`decay`](crate::decay) takes a caller-supplied chain rather than a context; a facility decays its buffer by popping, decaying and pushing back. |
//!
//! # Mass conservation on a rejected push
//!
//! Upstream throws on a push that would overflow, and the caller still holds
//! its `shared_ptr` — nothing is lost. Here the resource is *moved* into
//! [`push`](ResBuf::push), so an ordinary `Result<()>` would **destroy
//! material on the error path**, which is precisely the class of bug this
//! module must not have. Both push methods therefore fail with
//! [`Rejected`], which carries the resources back out. `?` still works in a
//! function returning [`Result`], because [`Rejected`] converts into
//! [`CyclusError`] — but that conversion is where the mass goes, so prefer
//! matching on it.

use alloc::vec::Vec;
use alloc::{collections::VecDeque, vec};

use crate::composition::AtomicMasses;
use crate::error::{CyclusError, Result};
use crate::limits::EPS_RSRC;
use crate::resource::Resource;

/// Resources handed back by a [`ResBuf`] push that could not be accepted.
///
/// Carries the reason *and* the resources, so no mass is lost on the error
/// path. See the [module docs](self) for why this exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    error: CyclusError,
    resources: Vec<Resource>,
}

impl Rejected {
    /// The reason the push was refused.
    #[must_use]
    pub fn error(&self) -> &CyclusError {
        &self.error
    }

    /// Borrows the refused resources, in the order they were offered.
    #[must_use]
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }

    /// Takes the refused resources back.
    #[must_use]
    pub fn into_resources(self) -> Vec<Resource> {
        self.resources
    }

    /// Takes back the single refused resource, for a rejected
    /// [`ResBuf::push`].
    ///
    /// Returns `None` if the rejection carried anything other than exactly one
    /// resource — i.e. if it came from [`ResBuf::push_all`].
    #[must_use]
    pub fn into_resource(self) -> Option<Resource> {
        let mut rs = self.resources;
        if rs.len() == 1 {
            rs.pop()
        } else {
            None
        }
    }
}

impl From<Rejected> for CyclusError {
    /// Discards the refused resources and keeps the reason, so that `?` works
    /// in a function returning [`Result`].
    ///
    /// **This is where mass is lost.** Use it only when the caller has already
    /// decided the refused resource is not wanted.
    fn from(r: Rejected) -> Self {
        r.error
    }
}

/// A push result: `Ok(())`, or the refused resources with their reason.
pub type PushResult = core::result::Result<(), Rejected>;

/// A FIFO inventory buffer of [`Resource`]s with a mass capacity, in
/// kilograms. Upstream `ResBuf<T>`.
///
/// Resources come out in the order they went in — oldest first — unless
/// [`pop_back`](ResBuf::pop_back) is used. A freshly constructed buffer has
/// **infinite** capacity, as upstream's does; call
/// [`set_capacity`](ResBuf::set_capacity) or build it with
/// [`with_capacity`](ResBuf::with_capacity).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResBuf {
    /// Running total in kilograms. Maintained incrementally and corrected by
    /// `update_qty` when the buffer empties or reduces to one object, exactly
    /// as upstream does — see [`ResBuf::quantity`].
    qty: f64,
    cap: f64,
    rs: VecDeque<Resource>,
}

impl ResBuf {
    /// An empty buffer of infinite capacity. Upstream `ResBuf()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            qty: 0.0,
            cap: f64::INFINITY,
            rs: VecDeque::new(),
        }
    }

    /// An empty buffer holding at most `cap` kilograms.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `cap` is negative or NaN.
    pub fn with_capacity(cap: f64) -> Result<Self> {
        let mut b = Self::new();
        b.set_capacity(cap)?;
        Ok(b)
    }

    /// The maximum mass this buffer can hold, in kilograms. Upstream
    /// `capacity()`. Never fails.
    ///
    /// `f64::INFINITY` for an unconstrained buffer.
    #[must_use]
    pub fn capacity(&self) -> f64 {
        self.cap
    }

    /// Sets the maximum mass, in kilograms. Upstream `capacity(double)`.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] if `cap` is negative or NaN.
    /// - [`CyclusError::Value`] if `cap` is below the mass already held, by
    ///   more than [`EPS_RSRC`]. Shrinking a buffer to *just* under its
    ///   contents is tolerated, because a long run accumulates rounding of
    ///   exactly that size; shrinking it meaningfully below is a modelling
    ///   error and must not silently orphan material.
    pub fn set_capacity(&mut self, cap: f64) -> Result<()> {
        if cap.is_nan() {
            return Err(CyclusError::Value("capacity must not be NaN"));
        }
        if cap < 0.0 {
            return Err(CyclusError::Value("capacity must not be negative"));
        }
        if self.quantity() - cap > EPS_RSRC {
            return Err(CyclusError::Value(
                "new capacity is lower than the quantity already held",
            ));
        }
        self.cap = cap;
        Ok(())
    }

    /// The number of resource objects held, dimensionless. Upstream `count()`.
    ///
    /// Not a mass: two 5 kg objects and one 10 kg object both weigh 10 kg but
    /// count 2 and 1.
    #[must_use]
    pub fn count(&self) -> usize {
        self.rs.len()
    }

    /// The total mass held, in kilograms. Upstream `quantity()`.
    ///
    /// # Why this is a running total and not a sum
    ///
    /// Upstream maintains `qty_` incrementally and re-derives it only when the
    /// buffer holds zero or one object (`UpdateQty`). That is reproduced here,
    /// drift and all, because the drift is bounded: any sequence of pushes and
    /// pops that empties the buffer resets the total to exactly zero, and a
    /// buffer holding one object reports that object's own quantity. A facility
    /// cycling material through a buffer therefore cannot accumulate error
    /// indefinitely.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        self.qty
    }

    /// The mass that can still be pushed, in kilograms, never negative.
    /// Upstream `space()`.
    #[must_use]
    pub fn space(&self) -> f64 {
        let s = self.cap - self.qty;
        if s > 0.0 {
            s
        } else {
            0.0
        }
    }

    /// `true` if the buffer holds no resource objects. Upstream `empty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rs.is_empty()
    }

    /// Borrows the held resources, oldest first.
    ///
    /// **Not an upstream member** — upstream's `rs_` is private with no
    /// accessor. Provided for inspection and reporting; it cannot be used to
    /// remove anything, so the running total stays correct.
    #[must_use]
    pub fn resources(&self) -> &VecDeque<Resource> {
        &self.rs
    }

    /// The next resource to be popped, without removing it. Upstream `Peek()`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if the buffer is empty.
    pub fn peek(&self) -> Result<&Resource> {
        self.rs
            .front()
            .ok_or(CyclusError::Value("cannot peek into an empty buffer"))
    }

    /// Adds one resource. Upstream `Push(Resource::Ptr)`.
    ///
    /// # Errors
    ///
    /// Returns [`Rejected`] — carrying `r` back — if `r`'s mass exceeds the
    /// remaining [`space`](ResBuf::space) by more than [`EPS_RSRC`]. The
    /// epsilon is upstream's and is what lets a facility push exactly
    /// `space()` kilograms after a long run has put a part-per-million of
    /// rounding into the running total.
    pub fn push(&mut self, r: Resource) -> PushResult {
        if r.quantity() - self.space() > EPS_RSRC {
            return Err(Rejected {
                error: CyclusError::Value("pushing this resource breaks the buffer capacity"),
                resources: vec![r],
            });
        }
        self.qty += r.quantity();
        self.rs.push_back(r);
        self.update_qty();
        Ok(())
    }

    /// Adds several resources, all or nothing. Upstream
    /// `Push(std::vector<B>)`.
    ///
    /// The capacity check is made against the **total** mass offered, so a
    /// batch that fits only partly is refused entirely and the buffer is left
    /// untouched — upstream's documented behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`Rejected`] — carrying every offered resource back, in order —
    /// if the total mass exceeds the remaining [`space`](ResBuf::space) by
    /// more than [`EPS_RSRC`].
    pub fn push_all(&mut self, rs: Vec<Resource>) -> PushResult {
        let mut tot = 0.0;
        for r in &rs {
            tot += r.quantity();
        }
        if tot - self.space() > EPS_RSRC {
            return Err(Rejected {
                error: CyclusError::Value("pushing these resources breaks the buffer capacity"),
                resources: rs,
            });
        }
        for r in rs {
            self.qty += r.quantity();
            self.rs.push_back(r);
        }
        self.update_qty();
        Ok(())
    }

    /// Removes and returns exactly `qty` kilograms, splitting a resource if
    /// needed. Upstream `PopVector(double)`.
    ///
    /// **This is the method every fuel-cycle mass balance runs through, so its
    /// contract is stated exactly.** Resources are taken oldest first. If the
    /// oldest resource is larger than what remains to be taken, it is *split*:
    /// the requested part is extracted and returned, and the remainder is put
    /// back at the **front** of the queue, keeping FIFO order. The returned
    /// pieces therefore sum to `qty`, and the buffer's
    /// [`quantity`](ResBuf::quantity) drops by exactly `qty`.
    ///
    /// # Parameters
    ///
    /// - `qty` — kilograms to remove, in `[0, quantity()]`.
    ///
    /// # Returns
    ///
    /// The removed pieces, oldest first. A request of `0.0` returns an empty
    /// vector and changes nothing.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] if `qty` exceeds [`quantity`](ResBuf::quantity).
    ///   **No tolerance is applied** — upstream's `PopVector` compares
    ///   strictly, and a caller that needs slack must use
    ///   [`pop_qty_eps`](ResBuf::pop_qty_eps), which is upstream's
    ///   `Pop(qty, eps)`. Getting this the wrong way round is how a buffer
    ///   ends up reporting a tiny negative inventory.
    /// - [`CyclusError::Value`] if `qty` is negative or NaN. Upstream does not
    ///   check: its `while (left > 0)` guard silently returns an empty vector
    ///   for a negative request. This port rejects it, because a negative pop
    ///   is a caller bug and returning "nothing" hides it.
    pub fn pop_qty(&mut self, qty: f64) -> Result<Vec<Resource>> {
        if qty.is_nan() {
            return Err(CyclusError::Value("removal quantity must not be NaN"));
        }
        if qty < 0.0 {
            return Err(CyclusError::Value("cannot remove a negative quantity"));
        }
        if qty > self.qty {
            return Err(CyclusError::Value(
                "removal quantity larger than the buffer quantity",
            ));
        }

        let mut out = Vec::new();
        let mut left = qty;
        while left > 0.0 && !self.rs.is_empty() {
            // `expect` is unreachable: the loop guard just checked non-empty.
            let mut r = self
                .rs
                .pop_front()
                .ok_or(CyclusError::State("buffer emptied concurrently"))?;
            let quan = r.quantity();
            let taken = if quan > left {
                // Too big: split, and put the remainder back at the front.
                let piece = r.extract_qty(left)?;
                self.rs.push_front(r);
                piece
            } else {
                r
            };
            self.qty -= taken.quantity();
            out.push(taken);
            // Upstream subtracts the ORIGINAL front quantity, not the piece's.
            // When a split happened `quan > left`, so `left` goes negative and
            // the loop ends — which is the intent either way.
            left -= quan;
        }

        self.update_qty();
        Ok(out)
    }

    /// [`pop_qty`](ResBuf::pop_qty) with a caller-supplied tolerance. Upstream
    /// `Pop(double qty, double eps)`.
    ///
    /// Use this when `qty` was computed from the buffer's own quantity and may
    /// exceed it by a rounding error. If `qty` is at or above the held mass
    /// (but within `eps` of it), the **whole buffer** is returned, unsplit —
    /// upstream's behaviour, and the reason this is not simply
    /// [`pop_qty`](ResBuf::pop_qty) with a looser comparison.
    ///
    /// # Parameters
    ///
    /// - `qty` — kilograms to remove.
    /// - `eps` — kilograms of slack allowed above the held mass;
    ///   [`EPS_RSRC`] is the usual choice.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` exceeds `quantity() + eps`, or if `qty`
    /// is negative or NaN.
    pub fn pop_qty_eps(&mut self, qty: f64, eps: f64) -> Result<Vec<Resource>> {
        if qty.is_nan() {
            return Err(CyclusError::Value("removal quantity must not be NaN"));
        }
        if qty < 0.0 {
            return Err(CyclusError::Value("cannot remove a negative quantity"));
        }
        if qty > self.qty + eps {
            return Err(CyclusError::Value(
                "removal quantity larger than the buffer quantity",
            ));
        }
        if qty >= self.qty {
            return self.pop_n(self.count());
        }
        self.pop_qty(qty)
    }

    /// Removes and returns the `n` oldest resource objects, unsplit. Upstream
    /// `PopN(int)`.
    ///
    /// # Parameters
    ///
    /// - `n` — a dimensionless object count, at most [`count`](ResBuf::count).
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `n` exceeds [`count`](ResBuf::count).
    /// (Upstream also rejects `n < 0`; `usize` makes that unrepresentable.)
    pub fn pop_n(&mut self, n: usize) -> Result<Vec<Resource>> {
        if n > self.count() {
            return Err(CyclusError::Value(
                "removal count larger than the buffer count",
            ));
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let r = self
                .rs
                .pop_front()
                .ok_or(CyclusError::State("buffer emptied concurrently"))?;
            self.qty -= r.quantity();
            out.push(r);
        }
        self.update_qty();
        Ok(out)
    }

    /// Removes and returns the **oldest** resource object, unsplit. Upstream
    /// `Pop()`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if the buffer is empty.
    pub fn pop(&mut self) -> Result<Resource> {
        let r = self
            .rs
            .pop_front()
            .ok_or(CyclusError::Value("cannot pop from an empty buffer"))?;
        self.qty -= r.quantity();
        self.update_qty();
        Ok(r)
    }

    /// Removes and returns the **most recently pushed** resource object,
    /// unsplit. Upstream `PopBack()`.
    ///
    /// This is the one method that does not respect FIFO order, and it is here
    /// because a facility that pushed something it then decides not to keep
    /// needs to take back that object rather than the oldest one.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if the buffer is empty.
    pub fn pop_back(&mut self) -> Result<Resource> {
        let r = self
            .rs
            .pop_back()
            .ok_or(CyclusError::Value("cannot pop from an empty buffer"))?;
        self.qty -= r.quantity();
        self.update_qty();
        Ok(r)
    }

    /// Combines everything held into a single resource object, in place.
    ///
    /// **Not an upstream member.** It is how this port offers the effect of
    /// upstream's bulk (`is_bulk_`) buffers and of `res_manip::Squash` without
    /// a global nuclear-data table — see the [module docs](self). The total
    /// mass is unchanged, so the capacity cannot be violated; only
    /// [`count`](ResBuf::count) changes, to `1` (or `0` for an empty buffer).
    ///
    /// # Parameters
    ///
    /// - `masses` — atomic masses, needed to mass-weight the merged
    ///   compositions. See
    ///   [`Material::absorb`](crate::material::Material::absorb).
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] if the buffer mixes materials and products,
    ///   which cannot be combined.
    /// - Whatever [`Material::absorb`](crate::material::Material::absorb)
    ///   reports — notably if two materials have been decayed to different
    ///   times.
    ///
    /// On error the buffer is left holding whatever had already been merged
    /// plus the untouched remainder, so no mass is lost.
    pub fn squash(&mut self, masses: &AtomicMasses) -> Result<()> {
        if self.rs.len() < 2 {
            return Ok(());
        }
        let all = self.pop_n(self.count())?;
        let one = squash_all(all, masses)?;
        self.qty += one.quantity();
        self.rs.push_back(one);
        self.update_qty();
        Ok(())
    }

    /// Upstream `UpdateQty`: re-derive the running total when it can be known
    /// exactly, so rounding cannot accumulate without bound.
    fn update_qty(&mut self) {
        match self.rs.len() {
            0 => self.qty = 0.0,
            1 => self.qty = self.rs[0].quantity(),
            _ => {}
        }
    }
}

/// Combines a list of resources into one. Upstream `res_manip::Squash`.
///
/// Materials are merged with
/// [`Material::absorb`](crate::material::Material::absorb), which mass-weights
/// the compositions; products are merged with
/// [`Product::absorb`](crate::product::Product::absorb), which requires equal
/// quality strings. The result's mass is the sum of the inputs' masses.
///
/// # Parameters
///
/// - `rs` — the resources to merge. Must be all materials or all products.
/// - `masses` — atomic masses in u, for the material case; ignored for
///   products.
///
/// # Errors
///
/// - [`CyclusError::Value`] if `rs` is empty — there is nothing to return.
/// - [`CyclusError::Value`] if `rs` mixes materials and products.
/// - Whatever the underlying `absorb` reports.
pub fn squash_all(rs: Vec<Resource>, masses: &AtomicMasses) -> Result<Resource> {
    let mut it = rs.into_iter();
    let first = it
        .next()
        .ok_or(CyclusError::Value("cannot squash an empty resource list"))?;
    match first {
        Resource::Material(mut acc) => {
            for r in it {
                let m = r.into_material()?;
                acc.absorb(m, masses)?;
            }
            Ok(Resource::Material(acc))
        }
        Resource::Product(mut acc) => {
            for r in it {
                let p = r.into_product()?;
                acc.absorb(p)?;
            }
            Ok(Resource::Product(acc))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::composition::Composition;
    use crate::limits::abs;
    use crate::material::Material;
    use crate::nuclide::nuc;
    use crate::product::Product;

    fn comp() -> Composition {
        let map: CompMap = [(nuc::U235, 0.05), (nuc::U238, 0.95)]
            .into_iter()
            .collect();
        Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap()
    }

    fn mat(qty: f64) -> Resource {
        Resource::from(Material::new(qty, comp()).unwrap())
    }

    /// Total mass held by a list of resources, in kilograms.
    fn total(rs: &[Resource]) -> f64 {
        rs.iter().map(Resource::quantity).sum()
    }

    // --- capacity ---------------------------------------------------------

    #[test]
    fn a_new_buffer_is_empty_with_infinite_capacity() {
        let b = ResBuf::new();
        assert!(b.is_empty());
        assert_eq!(b.count(), 0);
        assert_eq!(b.quantity(), 0.0);
        assert_eq!(b.capacity(), f64::INFINITY);
        assert_eq!(b.space(), f64::INFINITY);
    }

    #[test]
    fn capacity_is_enforced_on_push() {
        let mut b = ResBuf::with_capacity(100.0).unwrap();
        b.push(mat(60.0)).unwrap();
        assert_eq!(b.space(), 40.0);

        // 50 kg into 40 kg of space is refused, and comes back.
        let rejected = b.push(mat(50.0)).unwrap_err();
        assert_eq!(b.quantity(), 60.0);
        assert_eq!(b.count(), 1);
        let back = rejected.into_resource().unwrap();
        assert_eq!(back.quantity(), 50.0);

        // Exactly the remaining space is accepted.
        b.push(mat(40.0)).unwrap();
        assert!(abs(b.quantity() - 100.0) < 1e-12);
        assert_eq!(b.space(), 0.0);
    }

    #[test]
    fn push_tolerates_an_eps_sized_overshoot() {
        let mut b = ResBuf::with_capacity(100.0).unwrap();
        b.push(mat(100.0 + EPS_RSRC / 2.0)).unwrap();
        // Over by more than eps is refused.
        let mut c = ResBuf::with_capacity(100.0).unwrap();
        assert!(c.push(mat(100.0 + EPS_RSRC * 10.0)).is_err());
    }

    #[test]
    fn capacity_cannot_be_set_below_the_contents() {
        let mut b = ResBuf::with_capacity(100.0).unwrap();
        b.push(mat(80.0)).unwrap();
        assert!(b.set_capacity(50.0).is_err());
        assert_eq!(b.capacity(), 100.0, "a refused resize must not take effect");
        // An eps-sized shrink below the contents is tolerated.
        b.set_capacity(80.0 - EPS_RSRC / 2.0).unwrap();
        // And a resize upward always works.
        b.set_capacity(500.0).unwrap();
        assert_eq!(b.space(), 420.0);
    }

    #[test]
    fn capacity_rejects_negative_and_nan() {
        assert!(ResBuf::with_capacity(-1.0).is_err());
        assert!(ResBuf::with_capacity(f64::NAN).is_err());
    }

    #[test]
    fn push_all_is_all_or_nothing() {
        let mut b = ResBuf::with_capacity(100.0).unwrap();
        let batch = vec![mat(40.0), mat(40.0), mat(40.0)];
        let rejected = b.push_all(batch).unwrap_err();
        assert!(b.is_empty(), "a refused batch must leave the buffer empty");
        let back = rejected.into_resources();
        assert_eq!(back.len(), 3);
        assert!(abs(total(&back) - 120.0) < 1e-12, "no mass lost on refusal");

        // A batch that fits goes in whole.
        b.push_all(vec![mat(40.0), mat(40.0)]).unwrap();
        assert_eq!(b.count(), 2);
        assert!(abs(b.quantity() - 80.0) < 1e-12);
    }

    // --- FIFO and pop -----------------------------------------------------

    #[test]
    fn resources_come_out_oldest_first() {
        let mut b = ResBuf::new();
        for q in [1.0, 2.0, 3.0] {
            b.push(mat(q)).unwrap();
        }
        assert_eq!(b.peek().unwrap().quantity(), 1.0);
        assert_eq!(b.pop().unwrap().quantity(), 1.0);
        assert_eq!(b.pop().unwrap().quantity(), 2.0);
        assert_eq!(b.pop().unwrap().quantity(), 3.0);
        assert!(b.is_empty());
    }

    #[test]
    fn pop_back_returns_the_newest() {
        let mut b = ResBuf::new();
        for q in [1.0, 2.0, 3.0] {
            b.push(mat(q)).unwrap();
        }
        assert_eq!(b.pop_back().unwrap().quantity(), 3.0);
        assert_eq!(b.pop_back().unwrap().quantity(), 2.0);
        assert!(abs(b.quantity() - 1.0) < 1e-12);
    }

    #[test]
    fn popping_from_an_empty_buffer_is_an_error() {
        let mut b = ResBuf::new();
        assert!(b.pop().is_err());
        assert!(b.pop_back().is_err());
        assert!(b.peek().is_err());
    }

    #[test]
    fn pop_n_takes_whole_objects_and_rejects_an_overlarge_count() {
        let mut b = ResBuf::new();
        for q in [1.0, 2.0, 3.0] {
            b.push(mat(q)).unwrap();
        }
        assert!(b.pop_n(4).is_err());
        assert_eq!(b.count(), 3, "a refused pop_n must change nothing");

        let got = b.pop_n(2).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].quantity(), 1.0);
        assert_eq!(got[1].quantity(), 2.0);
        assert!(abs(b.quantity() - 3.0) < 1e-12);
    }

    // --- pop_qty: the mass-balance-critical path ---------------------------

    #[test]
    fn pop_qty_splits_a_resource_and_conserves_mass_exactly() {
        let mut b = ResBuf::new();
        b.push(mat(600.0)).unwrap();

        let got = b.pop_qty(250.0).unwrap();
        assert_eq!(got.len(), 1, "one split piece");
        assert_eq!(got[0].quantity(), 250.0);
        assert_eq!(b.count(), 1, "the remainder stays as one object");
        assert_eq!(b.quantity(), 350.0);
        // Nothing created, nothing destroyed.
        assert!(abs(total(&got) + b.quantity() - 600.0) < 1e-12);
    }

    #[test]
    fn pop_qty_spans_several_objects_and_splits_only_the_last() {
        let mut b = ResBuf::new();
        for q in [10.0, 20.0, 30.0] {
            b.push(mat(q)).unwrap();
        }
        // 45 kg: takes 10, takes 20, splits 15 off the 30.
        let got = b.pop_qty(45.0).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].quantity(), 10.0);
        assert_eq!(got[1].quantity(), 20.0);
        assert_eq!(got[2].quantity(), 15.0);
        assert!(abs(total(&got) - 45.0) < 1e-12);

        assert_eq!(b.count(), 1);
        assert!(abs(b.quantity() - 15.0) < 1e-12);
        assert!(abs(total(&got) + b.quantity() - 60.0) < 1e-12);
    }

    #[test]
    fn pop_qty_of_the_whole_inventory_empties_the_buffer_exactly() {
        let mut b = ResBuf::new();
        for q in [10.0, 20.0, 30.0] {
            b.push(mat(q)).unwrap();
        }
        let got = b.pop_qty(60.0).unwrap();
        assert_eq!(got.len(), 3);
        assert!(b.is_empty());
        assert_eq!(b.quantity(), 0.0, "an emptied buffer reports exactly zero");
        assert!(abs(total(&got) - 60.0) < 1e-12);
    }

    #[test]
    fn pop_qty_of_zero_is_a_no_op() {
        let mut b = ResBuf::new();
        b.push(mat(10.0)).unwrap();
        let got = b.pop_qty(0.0).unwrap();
        assert!(got.is_empty());
        assert_eq!(b.quantity(), 10.0);
        assert_eq!(b.count(), 1);
    }

    #[test]
    fn popping_more_than_is_held_is_an_error_and_changes_nothing() {
        let mut b = ResBuf::new();
        b.push(mat(10.0)).unwrap();
        assert!(b.pop_qty(10.0 + 1e-9).is_err(), "no tolerance in pop_qty");
        assert!(b.pop_qty(20.0).is_err());
        assert!(b.pop_qty(-1.0).is_err());
        assert!(b.pop_qty(f64::NAN).is_err());
        assert_eq!(b.quantity(), 10.0);
        assert_eq!(b.count(), 1);
    }

    #[test]
    fn pop_qty_eps_absorbs_a_rounding_overshoot_by_taking_everything() {
        let mut b = ResBuf::new();
        b.push(mat(10.0)).unwrap();
        b.push(mat(5.0)).unwrap();

        // A hair over the inventory: the whole buffer comes out, unsplit.
        let got = b.pop_qty_eps(15.0 + EPS_RSRC / 2.0, EPS_RSRC).unwrap();
        assert_eq!(got.len(), 2, "unsplit whole objects");
        assert!(abs(total(&got) - 15.0) < 1e-12);
        assert!(b.is_empty());

        // Beyond the tolerance it is still an error.
        let mut c = ResBuf::new();
        c.push(mat(10.0)).unwrap();
        assert!(c.pop_qty_eps(10.0 + EPS_RSRC * 10.0, EPS_RSRC).is_err());
        assert_eq!(c.quantity(), 10.0);
    }

    #[test]
    fn repeated_split_pops_conserve_mass_to_machine_precision() {
        // 100 kg taken out in 7 kg batches: 14 whole batches plus 2 kg.
        let mut b = ResBuf::new();
        b.push(mat(100.0)).unwrap();
        let mut taken = 0.0;
        for _ in 0..14 {
            let got = b.pop_qty(7.0).unwrap();
            taken += total(&got);
        }
        assert!(abs(taken - 98.0) < 1e-12, "taken {taken}");
        assert!(abs(b.quantity() - 2.0) < 1e-12, "left {}", b.quantity());
        let rest = b.pop_qty(b.quantity()).unwrap();
        taken += total(&rest);
        assert!(abs(taken - 100.0) < 1e-12, "total {taken}");
        assert_eq!(b.quantity(), 0.0);
    }

    #[test]
    fn space_bookkeeping_tracks_pushes_and_pops() {
        let mut b = ResBuf::with_capacity(100.0).unwrap();
        assert_eq!(b.space(), 100.0);
        b.push(mat(30.0)).unwrap();
        assert!(abs(b.space() - 70.0) < 1e-12);
        b.push(mat(70.0)).unwrap();
        assert_eq!(b.space(), 0.0);
        b.pop_qty(25.0).unwrap();
        assert!(abs(b.space() - 25.0) < 1e-12);
        b.pop_n(b.count()).unwrap();
        assert_eq!(b.space(), 100.0);
        assert_eq!(b.quantity(), 0.0);
    }

    // --- squash -----------------------------------------------------------

    #[test]
    fn squash_combines_materials_and_preserves_mass() {
        let mut b = ResBuf::new();
        b.push(mat(10.0)).unwrap();
        b.push(mat(20.0)).unwrap();
        b.push(mat(30.0)).unwrap();
        b.squash(&AtomicMasses::MassNumber).unwrap();
        assert_eq!(b.count(), 1);
        assert!(abs(b.quantity() - 60.0) < 1e-12);
        let m = b.peek().unwrap().as_material().unwrap();
        // Same composition throughout, so the enrichment is unchanged.
        assert!(abs(m.comp().mass_frac(nuc::U235) - 0.05) < 1e-12);
    }

    #[test]
    fn squash_refuses_to_mix_materials_and_products() {
        let mut b = ResBuf::new();
        b.push(mat(10.0)).unwrap();
        b.push(Resource::from(Product::new(5.0, "electricity").unwrap()))
            .unwrap();
        assert!(b.squash(&AtomicMasses::MassNumber).is_err());
    }

    #[test]
    fn squash_of_zero_or_one_object_is_a_no_op() {
        let mut b = ResBuf::new();
        b.squash(&AtomicMasses::MassNumber).unwrap();
        assert!(b.is_empty());
        b.push(mat(10.0)).unwrap();
        b.squash(&AtomicMasses::MassNumber).unwrap();
        assert_eq!(b.count(), 1);
        assert_eq!(b.quantity(), 10.0);
    }

    #[test]
    fn squash_all_rejects_an_empty_list() {
        assert!(squash_all(Vec::new(), &AtomicMasses::MassNumber).is_err());
    }

    // --- products ---------------------------------------------------------

    #[test]
    fn a_buffer_holds_products_as_readily_as_materials() {
        let mut b = ResBuf::with_capacity(100.0).unwrap();
        b.push(Resource::from(Product::new(40.0, "electricity").unwrap()))
            .unwrap();
        b.push(Resource::from(Product::new(40.0, "electricity").unwrap()))
            .unwrap();
        assert!(abs(b.quantity() - 80.0) < 1e-12);
        let got = b.pop_qty(50.0).unwrap();
        assert!(abs(total(&got) - 50.0) < 1e-12);
        assert!(abs(b.quantity() - 30.0) < 1e-12);
    }
}
