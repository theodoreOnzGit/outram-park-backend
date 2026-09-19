// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/context.h, src/context.cc (the recipe registry and
//                     the build/decommission scheduling half), src/timer.cc
//                     (the build and decommission queues)
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Simulation-global services: the clock, the recipe registry, and the
//! build/decommission schedule.
//!
//! # What upstream's `Context` does, and how it is split here
//!
//! Upstream's `Context` is the god object of a Cyclus simulation. It owns the
//! clock, the recipe registry, the prototype registry, the agent list, the
//! output recorder, the random number generator, the package and
//! transport-unit registries, and the build/decommission queues, and every
//! agent holds a `Context*` back-pointer to reach all of it.
//!
//! That shape does not translate. A back-pointer from every agent to a mutable
//! object that also *owns* those agents is the exact aliasing Rust exists to
//! prevent, and working around it would mean `Rc<RefCell<…>>` threaded through
//! the whole crate.
//!
//! So the responsibilities are split in two, along the line that makes each
//! half independently testable:
//!
//! | | owns | this crate |
//! |---|---|---|
//! | [`Context`] | clock, recipes, build/decom schedule | this module |
//! | `Simulation` | the agent arena and the phase loop | [`agents`](crate::agents) |
//!
//! An agent is then *handed* `&Context` for the duration of a phase call
//! rather than holding a pointer to it. That is a real interface change and it
//! is stated here rather than hidden: an agent cannot mutate the simulation
//! from inside `Tick`, it returns what it wants and the driver applies it.
//!
//! # Not carried over
//!
//! The output recorder, the prototype registry (Rust resolves archetypes at
//! compile time through an enum), and the `Package`/`TransportUnit`
//! registries. Packaging is a recent upstream feature that discretises a trade
//! into shippable units; it is a genuine feature gap here, noted in
//! [`agents`](crate::agents) rather than silently skipped.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::agent::AgentId;
use crate::composition::Composition;
use crate::error::{CyclusError, Result};
use crate::sim::{SimInfo, Timer};

/// A scheduled build: which prototype, under which parent, at which time step.
/// Upstream `Timer::build_queue_`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledBuild {
    /// The institution the new agent will sit under.
    pub parent: AgentId,
    /// The prototype name to build, as registered in the input deck.
    pub prototype: String,
    /// The time step at which to build it.
    pub time: i64,
}

/// A scheduled decommissioning. Upstream `Timer::decom_queue_`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledDecom {
    /// The agent to decommission.
    pub agent: AgentId,
    /// The time step at which to decommission it.
    pub time: i64,
}

/// Simulation-global state that agents read and schedule against.
///
/// Holds the clock, the named recipes, and the pending build and
/// decommission schedules. It deliberately does **not** hold the agents — see
/// this module's documentation for why.
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    timer: Timer,
    recipes: BTreeMap<String, Composition>,
    build_queue: Vec<ScheduledBuild>,
    decom_queue: Vec<ScheduledDecom>,
}

impl Context {
    /// Creates a context for the given simulation configuration.
    ///
    /// # Errors
    ///
    /// Whatever [`SimInfo::validate`] reports.
    pub fn new(info: SimInfo) -> Result<Self> {
        Ok(Self {
            timer: Timer::new(info)?,
            recipes: BTreeMap::new(),
            build_queue: Vec::new(),
            decom_queue: Vec::new(),
        })
    }

    /// The current time step. Upstream `Context::time()`.
    #[must_use]
    pub fn time(&self) -> i64 {
        self.timer.time()
    }

    /// The simulation configuration. Upstream `Context::sim_info()`.
    #[must_use]
    pub fn sim_info(&self) -> &SimInfo {
        self.timer.info()
    }

    /// The clock.
    #[must_use]
    pub fn timer(&self) -> &Timer {
        &self.timer
    }

    /// The clock, mutably — for the driver that advances it.
    pub fn timer_mut(&mut self) -> &mut Timer {
        &mut self.timer
    }

    // ---------------------------------------------------------------- recipes

    /// Registers a named recipe. Upstream `Context::AddRecipe`.
    ///
    /// A "recipe" is just a named [`Composition`] from the input deck —
    /// `"natural_u"`, `"spent_uox"`. Agents refer to them by name so an input
    /// deck can change a composition in one place.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `name` is already registered. Upstream silently
    /// overwrites, which means a duplicated recipe name in an input deck
    /// produces a simulation that depends on parse order. Rejecting it is a
    /// hardening; use [`Context::replace_recipe`] where an overwrite is
    /// intended.
    pub fn add_recipe(&mut self, name: &str, c: Composition) -> Result<()> {
        if self.recipes.contains_key(name) {
            return Err(CyclusError::Key("a recipe with that name already exists"));
        }
        self.recipes.insert(name.to_string(), c);
        Ok(())
    }

    /// Registers a named recipe, replacing any existing one of that name.
    ///
    /// Returns the composition that was replaced, if there was one. This is
    /// upstream's `AddRecipe` behaviour, available explicitly.
    pub fn replace_recipe(&mut self, name: &str, c: Composition) -> Option<Composition> {
        self.recipes.insert(name.to_string(), c)
    }

    /// Looks up a named recipe. Upstream `Context::GetRecipe`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if no recipe of that name is registered — the same
    /// `KeyError` upstream throws.
    pub fn recipe(&self, name: &str) -> Result<&Composition> {
        self.recipes
            .get(name)
            .ok_or(CyclusError::Key("no recipe registered under that name"))
    }

    /// `true` if a recipe of that name is registered.
    #[must_use]
    pub fn has_recipe(&self, name: &str) -> bool {
        self.recipes.contains_key(name)
    }

    /// The registered recipe names, in sorted order.
    #[must_use]
    pub fn recipe_names(&self) -> Vec<&str> {
        self.recipes.keys().map(String::as_str).collect()
    }

    // ------------------------------------------------------------- scheduling

    /// Schedules a prototype to be built under `parent` at time step `t`.
    /// Upstream `Context::SchedBuild`.
    ///
    /// Passing a `t` at or before the current time schedules it for the next
    /// step, matching upstream's `-1` default meaning "as soon as possible".
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `prototype` is empty.
    pub fn sched_build(&mut self, parent: AgentId, prototype: &str, t: i64) -> Result<()> {
        if prototype.is_empty() {
            return Err(CyclusError::Value("prototype name cannot be empty"));
        }
        let time = if t <= self.time() { self.time() + 1 } else { t };
        self.build_queue.push(ScheduledBuild {
            parent,
            prototype: prototype.to_string(),
            time,
        });
        Ok(())
    }

    /// Schedules `agent` to be decommissioned at time step `t`. Upstream
    /// `Context::SchedDecom`.
    ///
    /// A `t` at or before the current time schedules it for the current step,
    /// since decommissioning runs in the last phase and has therefore not yet
    /// happened.
    pub fn sched_decom(&mut self, agent: AgentId, t: i64) {
        let time = if t < self.time() { self.time() } else { t };
        self.decom_queue.push(ScheduledDecom { agent, time });
    }

    /// Removes and returns every build scheduled for time step `t`, in the
    /// order it was scheduled.
    ///
    /// Order is preserved — [`Vec::retain`] and the drain below are both
    /// stable — because two builds scheduled in the same step must happen in a
    /// deterministic order for the simulation to reproduce.
    pub fn take_builds_due(&mut self, t: i64) -> Vec<ScheduledBuild> {
        let mut due = Vec::new();
        let mut rest = Vec::new();
        for b in self.build_queue.drain(..) {
            if b.time == t {
                due.push(b);
            } else {
                rest.push(b);
            }
        }
        self.build_queue = rest;
        due
    }

    /// Removes and returns every decommissioning scheduled at or before time
    /// step `t`.
    ///
    /// "At or before", unlike [`take_builds_due`](Context::take_builds_due)'s
    /// exact match: a decommissioning whose step was somehow missed must still
    /// happen, whereas a build whose step has passed would silently change the
    /// deployment schedule.
    pub fn take_decoms_due(&mut self, t: i64) -> Vec<ScheduledDecom> {
        let mut due = Vec::new();
        let mut rest = Vec::new();
        for d in self.decom_queue.drain(..) {
            if d.time <= t {
                due.push(d);
            } else {
                rest.push(d);
            }
        }
        self.decom_queue = rest;
        due
    }

    /// The pending build schedule.
    #[must_use]
    pub fn build_queue(&self) -> &[ScheduledBuild] {
        &self.build_queue
    }

    /// The pending decommission schedule.
    #[must_use]
    pub fn decom_queue(&self) -> &[ScheduledDecom] {
        &self.decom_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::composition::AtomicMasses;
    use crate::nuclide::nuc;

    fn ctx() -> Context {
        Context::new(SimInfo::new(24)).unwrap()
    }

    fn natural_u() -> Composition {
        let map: CompMap = [(nuc::U235, 0.00711), (nuc::U238, 0.99289)]
            .into_iter()
            .collect();
        Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap()
    }

    #[test]
    fn a_recipe_round_trips_by_name() {
        let mut c = ctx();
        c.add_recipe("natural_u", natural_u()).unwrap();
        assert!(c.has_recipe("natural_u"));
        let got = c.recipe("natural_u").unwrap();
        assert!((got.mass_frac(nuc::U235) - 0.00711).abs() < 1e-12);
    }

    #[test]
    fn an_unknown_recipe_is_a_key_error() {
        let c = ctx();
        assert_eq!(
            c.recipe("spent_uox").unwrap_err(),
            CyclusError::Key("no recipe registered under that name")
        );
    }

    #[test]
    fn a_duplicate_recipe_name_is_rejected_rather_than_overwritten() {
        let mut c = ctx();
        c.add_recipe("feed", natural_u()).unwrap();
        assert_eq!(
            c.add_recipe("feed", natural_u()).unwrap_err(),
            CyclusError::Key("a recipe with that name already exists")
        );
        // The explicit form is available where an overwrite is intended.
        assert!(c.replace_recipe("feed", natural_u()).is_some());
    }

    #[test]
    fn recipe_names_are_sorted_so_iteration_reproduces() {
        let mut c = ctx();
        c.add_recipe("zulu", natural_u()).unwrap();
        c.add_recipe("alpha", natural_u()).unwrap();
        c.add_recipe("mike", natural_u()).unwrap();
        assert_eq!(c.recipe_names(), alloc::vec!["alpha", "mike", "zulu"]);
    }

    #[test]
    fn builds_come_due_only_on_their_exact_step() {
        let mut c = ctx();
        c.sched_build(AgentId(0), "LWR", 5).unwrap();
        c.sched_build(AgentId(0), "Mine", 7).unwrap();

        assert!(c.take_builds_due(4).is_empty());
        let due = c.take_builds_due(5);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].prototype, "LWR");
        assert_eq!(c.build_queue().len(), 1, "the step-7 build is still pending");
    }

    #[test]
    fn builds_due_in_the_same_step_keep_their_scheduling_order() {
        let mut c = ctx();
        for name in ["first", "second", "third"] {
            c.sched_build(AgentId(0), name, 3).unwrap();
        }
        let due = c.take_builds_due(3);
        let names: Vec<&str> = due.iter().map(|b| b.prototype.as_str()).collect();
        assert_eq!(names, alloc::vec!["first", "second", "third"]);
    }

    #[test]
    fn a_build_scheduled_in_the_past_moves_to_the_next_step() {
        let mut c = ctx();
        c.timer_mut().advance();
        c.timer_mut().advance(); // now at step 2
        c.sched_build(AgentId(0), "LWR", -1).unwrap();
        assert_eq!(c.build_queue()[0].time, 3);
    }

    #[test]
    fn an_empty_prototype_name_is_rejected() {
        let mut c = ctx();
        assert_eq!(
            c.sched_build(AgentId(0), "", 5).unwrap_err(),
            CyclusError::Value("prototype name cannot be empty")
        );
    }

    #[test]
    fn decommissionings_come_due_at_or_before_their_step() {
        let mut c = ctx();
        c.sched_decom(AgentId(1), 3);
        c.sched_decom(AgentId(2), 9);

        // A missed step must still fire, unlike a build.
        let due = c.take_decoms_due(5);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].agent, AgentId(1));
        assert_eq!(c.decom_queue().len(), 1);
    }

    #[test]
    fn a_decommissioning_scheduled_in_the_past_fires_this_step() {
        let mut c = ctx();
        c.timer_mut().advance();
        c.timer_mut().advance(); // step 2
        c.sched_decom(AgentId(1), 0);
        assert_eq!(c.decom_queue()[0].time, 2);
    }
}
