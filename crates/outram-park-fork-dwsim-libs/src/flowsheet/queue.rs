//! The flowsheet **calculation queue**: the FIFO of "this object needs
//! recalculating" requests that the sequential-modular solver drains.
//!
//! # What this represents
//!
//! When anything changes a flowsheet object — a user edits a pump's discharge
//! pressure, or a just-calculated unit operation writes a new outlet stream —
//! DWSIM pushes a [`CalculationArgs`] describing that object onto the flowsheet's
//! `CalculationQueue`, then asks the solver to run. The solver pops requests in
//! order and walks downstream from each one. The queue is therefore the
//! **hand-off point between the flowsheet data model and the solver**: this
//! module owns the data; the solver (a separate workstream) owns the draining.
//!
//! Nothing here is a physical quantity, so no `uom` types appear.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, not
//! the official DWSIM software.
//!
//! Source regions ported here:
//!
//! - `DWSIM.FlowsheetSolver/ObjectInfo.vb` lines 1-11 — the `CalculationArgs`
//!   class (`Sender`, `Calculated`, `Tag`, `Name`, `ObjectType`), ported as
//!   [`CalculationArgs`].
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` line 519 —
//!   `Public Property CalculationQueue As New Queue(Of ICalculationArgs)`,
//!   ported as [`CalculationQueue`] over a [`std::collections::VecDeque`].
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` lines 411-437 —
//!   `RequestCalculation`, whose *queueing half* (build a `CalculationArgs` from
//!   the sender and enqueue it) is ported as
//!   [`CalculationQueue::request_calculation`].
//!
//! # Excluded DWSIM behavior
//!
//! - **The solver call itself.** `RequestCalculation` finishes by invoking
//!   `FlowsheetSolver.SolveFlowsheet(...)`, optionally on a
//!   `Task.Factory.StartNew` background thread (FlowsheetBase.vb:427-435), and
//!   `RequestCalculation2` / `RequestCalculation3` /
//!   `RequestCalculationAndWait` / `Solve` (:405-465) are thin wrappers over
//!   that. None of it is ported here: the execution engine is the
//!   [`crate::flowsheet_solver`] workstream's, and the .NET task scheduling has
//!   no place in a Rust library — a caller chooses its own concurrency.
//! - **`ChangeCalculationOrder`** (FlowsheetBase.vb:3929-3943) — it exists only
//!   to open a WinForms dialog (`FormCustomCalcOrder`) and returns whatever the
//!   user dragged into place. The *data* it manipulates is an ordered list of
//!   object names, which a caller can simply reorder itself; the dialog is GUI
//!   and excluded.
//! - **`ICalculationArgs.Sender` as a magic string.** Upstream passes free-form
//!   sender strings such as `"PropertyGrid"` and `"FlowsheetSolver"`
//!   (FlowsheetBase.vb:422). The field is ported as an enum,
//!   [`CalculationSender`], with an `Other(String)` escape hatch, so the common
//!   cases are exhaustive-matchable while round-tripping stays possible.

use std::collections::VecDeque;

use crate::flowsheet::objects::{FlowsheetObject, ObjectType};

/// Who asked for a calculation — DWSIM's free-form `CalculationArgs.Sender`
/// string (ObjectInfo.vb:5), given a closed set of the values upstream actually
/// uses plus an escape hatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalculationSender {
    /// A user edit in the object's property editor — upstream `"PropertyGrid"`
    /// (FlowsheetBase.vb:422).
    PropertyGrid,
    /// The solver itself, propagating downstream — upstream
    /// `"FlowsheetSolver"`.
    FlowsheetSolver,
    /// A script or API call.
    Script,
    /// The dynamics integrator stepping the flowsheet in time.
    Integrator,
    /// Any other sender, preserved verbatim for round-tripping.
    Other(String),
}

impl CalculationSender {
    /// The upstream string this sender serialises to.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            CalculationSender::PropertyGrid => "PropertyGrid",
            CalculationSender::FlowsheetSolver => "FlowsheetSolver",
            CalculationSender::Script => "Script",
            CalculationSender::Integrator => "Integrator",
            CalculationSender::Other(s) => s,
        }
    }

    /// Parse an upstream sender string. Unrecognised values become
    /// [`CalculationSender::Other`] rather than being dropped.
    #[must_use]
    pub fn from_str_lossless(s: &str) -> Self {
        match s {
            "PropertyGrid" => CalculationSender::PropertyGrid,
            "FlowsheetSolver" => CalculationSender::FlowsheetSolver,
            "Script" => CalculationSender::Script,
            "Integrator" => CalculationSender::Integrator,
            other => CalculationSender::Other(other.to_string()),
        }
    }
}

/// One entry in the calculation queue — DWSIM's `CalculationArgs`
/// (ObjectInfo.vb:1-11).
///
/// Identifies **which** object needs calculating (by immutable `name`/ID and by
/// user-visible `tag`), **what kind** it is (so the solver can dispatch without
/// looking the object up), whether it has since been calculated, and **who**
/// asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculationArgs {
    /// Who requested the calculation (`CalculationArgs.Sender`).
    pub sender: CalculationSender,
    /// Whether this request has been satisfied (`CalculationArgs.Calculated`).
    /// DWSIM initialises it `False` (ObjectInfo.vb:6) and the solver flips it.
    pub calculated: bool,
    /// The target object's user-visible tag (`CalculationArgs.Tag`), e.g.
    /// `PUMP-1`.
    pub tag: String,
    /// The target object's immutable identity (`CalculationArgs.Name`) — the
    /// [`crate::flowsheet::objects::ObjectId`] string. This, not the tag, is
    /// what the registry is keyed by.
    pub name: String,
    /// The target object's type (`CalculationArgs.ObjectType`). Upstream
    /// defaults it to `ObjectType.Nenhum` (ObjectInfo.vb:9), which is
    /// [`ObjectType::Undefined`] here.
    pub object_type: ObjectType,
}

impl CalculationArgs {
    /// A request for `object`, attributed to `sender`, not yet calculated —
    /// exactly the four assignments DWSIM makes at FlowsheetBase.vb:416-423.
    #[must_use]
    pub fn for_object(object: &FlowsheetObject, sender: CalculationSender) -> Self {
        CalculationArgs {
            sender,
            calculated: false,
            tag: object.tag.clone(),
            name: object.id.0.clone(),
            object_type: object.object_type,
        }
    }
}

impl Default for CalculationArgs {
    /// DWSIM's field initialisers (ObjectInfo.vb:5-9): empty sender/tag/name,
    /// `Calculated = False`, `ObjectType = Nenhum`.
    fn default() -> Self {
        CalculationArgs {
            sender: CalculationSender::Other(String::new()),
            calculated: false,
            tag: String::new(),
            name: String::new(),
            object_type: ObjectType::Undefined,
        }
    }
}

/// The flowsheet's FIFO of pending calculation requests — DWSIM's
/// `CalculationQueue As New Queue(Of ICalculationArgs)`
/// (FlowsheetBase.vb:519).
///
/// A thin, deliberately boring wrapper over [`VecDeque`]: **enqueue at the back,
/// dequeue at the front**, matching .NET `Queue(Of T)` semantics exactly. It is
/// kept simple on purpose — the [`crate::flowsheet_solver`] workstream builds
/// its execution order on top of this type, so the contract needs to be
/// obvious.
///
/// Not thread-safe by itself; share it as `Arc<RwLock<CalculationQueue>>` if a
/// caller needs concurrent producers, per the workspace shared-state rule (never
/// channels).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalculationQueue {
    items: VecDeque<CalculationArgs>,
}

impl CalculationQueue {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        CalculationQueue {
            items: VecDeque::new(),
        }
    }

    /// Push a request onto the back of the queue (.NET `Queue.Enqueue`,
    /// FlowsheetBase.vb:425).
    pub fn enqueue(&mut self, args: CalculationArgs) {
        self.items.push_back(args);
    }

    /// Build a [`CalculationArgs`] for `object` and enqueue it — the queueing
    /// half of `RequestCalculation` (FlowsheetBase.vb:413-425). The solver
    /// invocation that follows upstream is **not** performed here (see the
    /// module's "Excluded DWSIM behavior").
    pub fn request_calculation(&mut self, object: &FlowsheetObject, sender: CalculationSender) {
        self.enqueue(CalculationArgs::for_object(object, sender));
    }

    /// Pop the oldest request (.NET `Queue.Dequeue`), or `None` if empty.
    ///
    /// Upstream `Dequeue` on an empty queue throws; returning `None` is the
    /// honest Rust equivalent.
    pub fn dequeue(&mut self) -> Option<CalculationArgs> {
        self.items.pop_front()
    }

    /// Look at the oldest request without removing it (.NET `Queue.Peek`).
    #[must_use]
    pub fn peek(&self) -> Option<&CalculationArgs> {
        self.items.front()
    }

    /// Number of pending requests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Discard every pending request (.NET `Queue.Clear`). DWSIM clears the
    /// queue when a solve is abandoned.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Iterate the pending requests oldest-first, without consuming them.
    pub fn iter(&self) -> impl Iterator<Item = &CalculationArgs> {
        self.items.iter()
    }

    /// Whether any pending request targets the object with this immutable
    /// identity. Useful for the solver's "already queued, don't queue again"
    /// check.
    #[must_use]
    pub fn contains_name(&self, name: &str) -> bool {
        self.items.iter().any(|a| a.name == name)
    }
}

#[cfg(test)]
mod tests {
    //! # Verification tests — calculation queue
    //!
    //! **Methodology.** Verification that the queue reproduces .NET
    //! `Queue(Of T)` FIFO semantics and that `request_calculation` copies the
    //! same four fields DWSIM copies at FlowsheetBase.vb:416-423. No physics is
    //! involved. Results recorded 2026-08-11.

    use super::*;
    use crate::flowsheet::objects::ObjectId;

    fn pump() -> FlowsheetObject {
        FlowsheetObject::new(ObjectId::from("pump-guid"), "PUMP-1", ObjectType::Pump)
    }

    /// **Methodology.** `request_calculation` must copy tag, name and object
    /// type off the object and mark the request not-yet-calculated
    /// (FlowsheetBase.vb:416-423).
    /// **Result (2026-08-11):** tag `PUMP-1`, name `pump-guid`, type `Pump`,
    /// `calculated = false`, sender `PropertyGrid`.
    #[test]
    fn request_calculation_copies_object_identity() {
        let mut q = CalculationQueue::new();
        q.request_calculation(&pump(), CalculationSender::PropertyGrid);
        let a = q.peek().unwrap();
        assert_eq!(a.tag, "PUMP-1");
        assert_eq!(a.name, "pump-guid");
        assert_eq!(a.object_type, ObjectType::Pump);
        assert!(!a.calculated);
        assert_eq!(a.sender, CalculationSender::PropertyGrid);
        assert_eq!(a.sender.as_str(), "PropertyGrid");
    }

    /// **Methodology.** FIFO ordering, `len`/`is_empty`, `clear`, and
    /// `dequeue` on an empty queue returning `None` rather than panicking (.NET
    /// throws).
    /// **Result (2026-08-11):** three enqueues dequeue in insertion order;
    /// `clear` empties; a fourth `dequeue` returns `None`.
    #[test]
    fn queue_is_fifo_and_empty_dequeue_is_none() {
        let mut q = CalculationQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.dequeue(), None);

        for tag in ["A", "B", "C"] {
            let obj = FlowsheetObject::new(ObjectId::from(tag), tag, ObjectType::MaterialStream);
            q.request_calculation(&obj, CalculationSender::FlowsheetSolver);
        }
        assert_eq!(q.len(), 3);
        assert_eq!(q.dequeue().unwrap().tag, "A");
        assert_eq!(q.dequeue().unwrap().tag, "B");
        assert_eq!(q.len(), 1);
        assert!(q.contains_name("C"));
        assert!(!q.contains_name("A"));
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.dequeue(), None);
    }

    /// **Methodology.** [`CalculationArgs::default`] must match DWSIM's field
    /// initialisers (ObjectInfo.vb:5-9), and [`CalculationSender`] must
    /// round-trip through its upstream string form including unknown values.
    /// **Result (2026-08-11):** default is empty/false/`Undefined`;
    /// `"PropertyGrid"` and the unknown `"Macro"` both round-trip.
    #[test]
    fn defaults_and_sender_round_trip() {
        let d = CalculationArgs::default();
        assert!(!d.calculated);
        assert_eq!(d.tag, "");
        assert_eq!(d.name, "");
        assert_eq!(d.object_type, ObjectType::Undefined);

        for s in [
            "PropertyGrid",
            "FlowsheetSolver",
            "Script",
            "Integrator",
            "Macro",
        ] {
            assert_eq!(CalculationSender::from_str_lossless(s).as_str(), s);
        }
        assert_eq!(
            CalculationSender::from_str_lossless("Macro"),
            CalculationSender::Other("Macro".to_string())
        );
    }

    /// **Methodology.** `iter` must yield oldest-first without consuming.
    /// **Result (2026-08-11):** iteration order A, B; length unchanged at 2.
    #[test]
    fn iteration_is_non_consuming_and_ordered() {
        let mut q = CalculationQueue::new();
        for tag in ["A", "B"] {
            let obj = FlowsheetObject::new(ObjectId::from(tag), tag, ObjectType::Valve);
            q.request_calculation(&obj, CalculationSender::Script);
        }
        let tags: Vec<&str> = q.iter().map(|a| a.tag.as_str()).collect();
        assert_eq!(tags, vec!["A", "B"]);
        assert_eq!(q.len(), 2);
    }
}
