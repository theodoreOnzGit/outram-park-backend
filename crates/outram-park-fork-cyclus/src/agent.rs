// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/agent.h, src/agent.cc, src/region.h,
//                     src/institution.h, src/facility.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Agent identity, the institutional hierarchy, and agent lifetimes.
//!
//! # The three-level hierarchy
//!
//! A Cyclus simulation is a tree, exactly three levels deep below the root:
//!
//! ```text
//!   Region            a country, or a market
//!     Institution     an owner/operator — decides what gets built
//!       Facility      a mine, enrichment plant, reactor, repository
//! ```
//!
//! Only [`AgentRole::Facility`] agents trade material. Institutions decide
//! what to build and when to decommission; regions set the demand the
//! institutions build against. The hierarchy is not decoration — an
//! institution can only build facilities beneath itself, and a region's growth
//! target only reaches the facilities under it.
//!
//! # Identity without pointers
//!
//! Upstream every agent holds `Agent* parent_` and `std::set<Agent*>
//! children_`. This workspace forbids that shape, so parentage is stored as
//! [`AgentId`] indices into the [`Context`](crate::context::Context)'s arena.
//! The pattern is the one the workspace `CLAUDE.md` names explicitly for graph
//! and topology links, and it buys something beyond compliance: an
//! [`AgentInfo`] is `Copy`-cheap to pass around and can be compared, stored
//! and serialised without the aliasing questions a raw pointer raises.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{CyclusError, Result};

/// A handle to an agent, an index into the simulation's agent arena.
///
/// Upstream's `Agent::id()`, which is an `int` assigned from a global counter.
/// Here it is an arena index, so it doubles as the lookup key and no separate
/// map is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId(pub usize);

impl AgentId {
    /// The underlying index.
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Which level of the hierarchy an agent occupies. Upstream `Agent::kind()`,
/// which returns the string `"Region"`, `"Inst"` or `"Facility"`.
///
/// An enum rather than a string, so `ancestor_of_kind` cannot be asked for a
/// level that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentRole {
    /// A country or market. Upstream kind string `"Region"`.
    Region,
    /// An owner/operator that builds and decommissions facilities. Upstream
    /// kind string `"Inst"`.
    Institution,
    /// A physical facility that holds and trades material. Upstream kind
    /// string `"Facility"`.
    Facility,
}

impl AgentRole {
    /// The upstream kind string.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Region => "Region",
            Self::Institution => "Inst",
            Self::Facility => "Facility",
        }
    }

    /// The role this one must sit beneath, or `None` for
    /// [`AgentRole::Region`], which sits at the root.
    #[must_use]
    pub fn parent_role(self) -> Option<Self> {
        match self {
            Self::Region => None,
            Self::Institution => Some(Self::Region),
            Self::Facility => Some(Self::Institution),
        }
    }

    /// `true` if an agent of this role may be a child of `parent`.
    ///
    /// Upstream does not enforce this — `Agent::Connect` accepts any parent
    /// and a malformed input deck produces a simulation whose hierarchy is
    /// quietly wrong. Making it checkable here is a hardening, and
    /// [`Context`](crate::context::Context) rejects a build that violates it.
    #[must_use]
    pub fn may_sit_under(self, parent: Option<Self>) -> bool {
        self.parent_role() == parent
    }
}

/// How long an agent lives, in time steps. Upstream `Agent::lifetime()`,
/// which uses `-1` as its infinite sentinel.
///
/// An enum, because `-1` meaning "forever" is precisely the kind of in-band
/// sentinel that gets compared with `<` by accident. Upstream's integer form
/// is still available through [`Lifetime::as_upstream_int`] and
/// [`Lifetime::from_upstream_int`] so a configuration round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lifetime {
    /// The agent is never decommissioned on account of age. Upstream `-1`.
    #[default]
    Forever,
    /// The agent lives this many time steps. Must be positive.
    Steps(i64),
}

impl Lifetime {
    /// Upstream's integer encoding: `-1` for infinite.
    #[must_use]
    pub fn as_upstream_int(self) -> i64 {
        match self {
            Self::Forever => -1,
            Self::Steps(n) => n,
        }
    }

    /// Decodes upstream's integer encoding.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] for `0`, or for any negative value other than
    /// `-1`. Upstream accepts them and produces an exit time in the past.
    pub fn from_upstream_int(n: i64) -> Result<Self> {
        match n {
            -1 => Ok(Self::Forever),
            n if n > 0 => Ok(Self::Steps(n)),
            _ => Err(CyclusError::Value(
                "lifetime must be positive, or -1 for infinite",
            )),
        }
    }
}

/// The bookkeeping every agent carries, whatever its archetype.
///
/// Upstream this is the state on the `Agent` base class. Here it is a plain
/// struct that each archetype *contains*, rather than inherits — the
/// composition equivalent of upstream's inheritance, and the shape that lets
/// [`AgentKind`](crate::agents::AgentKind) stay a flat enum.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentInfo {
    /// This agent's handle.
    pub id: AgentId,
    /// Its parent in the hierarchy, or `None` for a region.
    pub parent: Option<AgentId>,
    /// Its children, in the order they were built.
    pub children: Vec<AgentId>,
    /// Which level of the hierarchy it occupies.
    pub role: AgentRole,
    /// The user-facing prototype name from the input deck, e.g.
    /// `"LWR"`. Upstream `Agent::prototype()`.
    pub prototype: String,
    /// The archetype specification, e.g. `":cycamore:Reactor"`. Upstream
    /// `Agent::spec()`.
    pub spec: String,
    /// The time step at which the agent entered the simulation. Upstream
    /// `Agent::enter_time()`.
    pub enter_time: i64,
    /// How long it lives.
    pub lifetime: Lifetime,
    /// `true` once it has been decommissioned.
    pub decommissioned: bool,
}

impl AgentInfo {
    /// Creates bookkeeping for an agent entering at `enter_time`.
    #[must_use]
    pub fn new(
        id: AgentId,
        role: AgentRole,
        prototype: &str,
        spec: &str,
        enter_time: i64,
    ) -> Self {
        Self {
            id,
            parent: None,
            children: Vec::new(),
            role,
            prototype: prototype.to_string(),
            spec: spec.to_string(),
            enter_time,
            lifetime: Lifetime::Forever,
            decommissioned: false,
        }
    }

    /// Sets the lifetime, consuming and returning `self`.
    #[must_use]
    pub fn with_lifetime(mut self, lifetime: Lifetime) -> Self {
        self.lifetime = lifetime;
        self
    }

    /// Sets the parent, consuming and returning `self`.
    #[must_use]
    pub fn with_parent(mut self, parent: AgentId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// The final time step of this agent's life, or `None` if it lives
    /// forever. Upstream `Agent::exit_time()`.
    ///
    /// # The off-by-one is upstream's, and is deliberate
    ///
    /// The exit time is `enter_time + lifetime - 1`, **not**
    /// `enter_time + lifetime`. Upstream's own comment explains why:
    /// decommissioning happens at the *end* of a time step, so an agent with a
    /// lifetime of 1 goes through exactly one whole step and is
    /// decommissioned on the step it was created. Reproducing the `- 1` is
    /// required for a deployment schedule to match upstream's.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_cyclus::agent::{AgentId, AgentInfo, AgentRole, Lifetime};
    ///
    /// let a = AgentInfo::new(AgentId(0), AgentRole::Facility, "LWR", ":cycamore:Reactor", 5)
    ///     .with_lifetime(Lifetime::Steps(1));
    /// // Enters at 5, lives one step, exits at 5.
    /// assert_eq!(a.exit_time(), Some(5));
    /// ```
    #[must_use]
    pub fn exit_time(&self) -> Option<i64> {
        match self.lifetime {
            Lifetime::Forever => None,
            Lifetime::Steps(n) => Some(self.enter_time + n - 1),
        }
    }

    /// `true` if the agent has reached or passed its exit time at time step
    /// `now`. Upstream `Agent::retired()`.
    ///
    /// Always `false` for an agent with an infinite lifetime.
    #[must_use]
    pub fn retired(&self, now: i64) -> bool {
        match self.exit_time() {
            None => false,
            Some(t) => now >= t,
        }
    }

    /// `true` if the agent is in the simulation at time step `now`: it has
    /// entered, has not been decommissioned, and has not passed its exit time.
    #[must_use]
    pub fn alive(&self, now: i64) -> bool {
        !self.decommissioned && now >= self.enter_time && !self.retired(now)
    }

    /// The number of time steps the agent has been in the simulation at `now`.
    ///
    /// Zero on the step it entered. Negative before it enters, which a caller
    /// should treat as "not yet built" rather than clamping.
    #[must_use]
    pub fn age(&self, now: i64) -> i64 {
        now - self.enter_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facility(enter: i64) -> AgentInfo {
        AgentInfo::new(
            AgentId(0),
            AgentRole::Facility,
            "LWR",
            ":cycamore:Reactor",
            enter,
        )
    }

    #[test]
    fn kind_strings_match_upstream() {
        assert_eq!(AgentRole::Region.as_str(), "Region");
        assert_eq!(AgentRole::Institution.as_str(), "Inst");
        assert_eq!(AgentRole::Facility.as_str(), "Facility");
    }

    #[test]
    fn hierarchy_nesting_is_enforced() {
        assert!(AgentRole::Region.may_sit_under(None));
        assert!(AgentRole::Institution.may_sit_under(Some(AgentRole::Region)));
        assert!(AgentRole::Facility.may_sit_under(Some(AgentRole::Institution)));

        // The ones upstream would silently accept.
        assert!(!AgentRole::Facility.may_sit_under(Some(AgentRole::Region)));
        assert!(!AgentRole::Region.may_sit_under(Some(AgentRole::Region)));
        assert!(!AgentRole::Institution.may_sit_under(None));
    }

    #[test]
    fn exit_time_reproduces_upstreams_off_by_one() {
        // Upstream: a lifetime of 1 means the agent exits on the step it
        // entered, because decommissioning happens at the END of a step.
        let a = facility(5).with_lifetime(Lifetime::Steps(1));
        assert_eq!(a.exit_time(), Some(5));

        let b = facility(5).with_lifetime(Lifetime::Steps(10));
        assert_eq!(b.exit_time(), Some(14));
    }

    #[test]
    fn an_infinite_lifetime_has_no_exit_time_and_never_retires() {
        let a = facility(0);
        assert_eq!(a.exit_time(), None);
        assert!(!a.retired(0));
        assert!(!a.retired(1_000_000));
    }

    #[test]
    fn retired_is_true_from_the_exit_step_onward() {
        let a = facility(5).with_lifetime(Lifetime::Steps(3)); // exits at 7
        assert!(!a.retired(6));
        assert!(a.retired(7));
        assert!(a.retired(8));
    }

    #[test]
    fn alive_covers_entry_decommissioning_and_retirement() {
        let mut a = facility(5).with_lifetime(Lifetime::Steps(3)); // exits at 7
        assert!(!a.alive(4), "not yet built");
        assert!(a.alive(5));
        assert!(a.alive(6));
        assert!(!a.alive(7), "retired");

        let mut b = facility(5);
        assert!(b.alive(6));
        b.decommissioned = true;
        assert!(!b.alive(6));

        a.decommissioned = true;
        assert!(!a.alive(6));
    }

    #[test]
    fn age_is_zero_on_the_entry_step() {
        let a = facility(5);
        assert_eq!(a.age(5), 0);
        assert_eq!(a.age(9), 4);
        assert_eq!(a.age(3), -2, "negative before entry, not clamped");
    }

    #[test]
    fn lifetime_round_trips_through_upstreams_integer_encoding() {
        assert_eq!(Lifetime::Forever.as_upstream_int(), -1);
        assert_eq!(Lifetime::from_upstream_int(-1).unwrap(), Lifetime::Forever);
        assert_eq!(
            Lifetime::from_upstream_int(12).unwrap(),
            Lifetime::Steps(12)
        );
    }

    #[test]
    fn lifetime_rejects_zero_and_other_negatives() {
        assert!(Lifetime::from_upstream_int(0).is_err());
        assert!(Lifetime::from_upstream_int(-2).is_err());
    }
}
