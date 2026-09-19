// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus> (the
//                     Agent/Facility/Trader interfaces and Timer::RunSim's
//                     phase loop) and CYCAMORE
//                     <https://github.com/cyclus/cycamore> (the archetypes).
//   Upstream files:   cyclus/src/{agent,facility,trader,trader_management,
//                     timer,exchange_manager}.h, cycamore/src/*.{h,cc}
//   Upstream commits: d4faab7ce0566ccb8febfcf50915cdeedee59db0 (cyclus),
//                     fee8c80190e0b91dafccae6b1a3129dc544441e8 (cycamore)
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Fuel-cycle facility agents, and the driver that runs a simulation of them.
//!
//! # The trading protocol, in the order it happens
//!
//! Every time step, [`Simulation::step`] runs the six phases of
//! [`Phase`](crate::sim::Phase). The middle one is the dynamic resource
//! exchange, and it is worth following once end to end because every agent in
//! this module is written against it:
//!
//! 1. **Requests.** Each agent returns [`RequestPortfolio`]s saying what it
//!    wants — a commodity, a quantity, a target composition, and a preference
//!    for each supplier. ([`Facility::material_requests`])
//! 2. **Bids.** Each agent is shown every request, grouped by commodity, and
//!    answers with [`BidPortfolio`]s. A bid names the request it answers and
//!    the resource it would supply, and its portfolio carries the supplier's
//!    capacity constraints. ([`Facility::material_bids`])
//! 3. **Solve.** The portfolios are translated into an
//!    [`ExchangeGraph`](crate::exchange::graph::ExchangeGraph) and matched by
//!    the [`GreedySolver`], which respects every constraint and prefers
//!    higher-preference arcs.
//! 4. **Respond.** Each winning supplier is told how much of what it must
//!    produce, and hands over real resources.
//!    ([`Facility::respond_to_trades`])
//! 5. **Accept.** Each requester receives them. ([`Facility::accept_trades`])
//!
//! Nothing moves outside that protocol. An agent cannot push material at
//! another agent, and cannot reach into the simulation — it is handed
//! `&Context` for the duration of a call and returns what it wants, which the
//! driver applies. See [`context`](crate::context) for why.
//!
//! # A trait for the contract, an enum for the dispatch
//!
//! [`Facility`] is a trait, so the compiler checks that every archetype
//! implements the whole protocol. It is **not** used for dispatch — the
//! workspace forbids trait objects. [`AgentKind`] is an enum over the concrete
//! archetypes and every call site matches on it.
//!
//! The payoff is not just rule compliance. Upstream resolves archetypes at run
//! time by loading shared libraries (`dynamic_module.cc`), so a typo in an
//! input deck's `spec` string is a run-time failure and adding a new
//! interface method silently breaks any archetype that does not override it.
//! Here, adding a variant to [`AgentKind`] makes the compiler point at every
//! `match` that must handle it.
//!
//! # What is implemented
//!
//! | Archetype | Status |
//! |---|---|
//! | [`Source`] | implemented |
//! | [`Sink`] | implemented |
//! | `Storage`, `Enrichment`, `Reactor`, `Separations`, `FuelFab`, `Mixer` | not yet ported |
//! | `DeployInst`, `ManagerInst`, `GrowthRegion` | not yet ported |
//!
//! The unported archetypes are absent rather than stubbed: a stub that
//! silently trades nothing is worse than a missing variant, because a
//! simulation built on it runs and produces a plausible, wrong answer.
//!
//! # Not ported: trade packaging
//!
//! Recent upstream splits a trade into `Package`-sized units and limits how
//! many `TransportUnit`s may ship per step. That changes the *numbers* a
//! simulation produces, not merely how they are recorded, so it is a genuine
//! feature gap rather than an infrastructure boundary. Agents here trade the
//! matched quantity in one piece. See `docs/port-notes.md`.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::agent::{AgentId, AgentInfo, AgentRole};
use crate::composition::AtomicMasses;
use crate::context::Context;
use crate::error::{CyclusError, Result};
use crate::exchange::{
    BidPortfolio, ExchangeContext, GreedySolver, Request, RequestId, RequestPortfolio, Trade,
};
use crate::resource::Resource;
use crate::sim::Phase;

pub mod sink;
pub mod source;

pub use sink::Sink;
pub use source::Source;

/// Every request in a time step, grouped by the commodity it asks for.
///
/// Upstream `cyclus::CommodMap<Material>::type`, which is
/// `std::map<std::string, std::vector<Request<Material>*>>`. Bidders are shown
/// this and answer the requests they can serve.
///
/// A `BTreeMap` rather than a hash map, so bidders see commodities in the same
/// order on every run and every platform — the same reproducibility argument
/// as [`CompMap`](crate::comp_math::CompMap).
pub type CommodRequests = BTreeMap<String, Vec<(RequestId, Request)>>;

/// One order a supplier has won and must now fill.
///
/// Upstream passes `std::vector<Trade<Material>>` and expects the supplier to
/// push `(trade, material)` pairs into an out-parameter. Returning the
/// resources is clearer and makes the "responded to the wrong trade" mistake
/// impossible, so [`Facility::respond_to_trades`] returns them in the same
/// order it was given the orders.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    /// The matched trade: which request, which bid, how much.
    pub trade: Trade,
    /// The commodity being supplied, for an agent that sells more than one.
    pub commodity: String,
    /// The composition the requester asked for, so a supplier that offers
    /// "whatever is requested" knows what to make.
    pub target: Resource,
}

/// One delivery a requester has received.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    /// The trade this fills.
    pub trade: Trade,
    /// The commodity delivered.
    pub commodity: String,
    /// The resource itself.
    pub resource: Resource,
}

/// The contract every facility archetype satisfies.
///
/// A compiler-enforced interface, **not** a dispatch mechanism — see the
/// module documentation. Every method has a default that does nothing, so an
/// archetype implements only the phases it participates in; a pure consumer
/// never writes a `material_bids`, and a pure producer never writes a
/// `material_requests`.
///
/// Every method takes `&Context` rather than holding one. An agent therefore
/// cannot mutate the simulation from inside a phase — it returns what it
/// wants and the driver applies it.
pub trait Facility {
    /// Update internal state at the start of a time step, before any trading.
    /// Upstream `Facility::Tick`.
    ///
    /// # Errors
    ///
    /// Archetype-specific; a misconfigured facility reports it here.
    fn tick(&mut self, _ctx: &Context) -> Result<()> {
        Ok(())
    }

    /// Process whatever was received, after trading. Upstream
    /// `Facility::Tock`.
    ///
    /// # Errors
    ///
    /// Archetype-specific.
    fn tock(&mut self, _ctx: &Context) -> Result<()> {
        Ok(())
    }

    /// Say what this agent wants this step. Upstream
    /// `Trader::GetMatlRequests`.
    ///
    /// # Errors
    ///
    /// Archetype-specific.
    fn material_requests(&mut self, _ctx: &Context, _me: AgentId) -> Result<Vec<RequestPortfolio>> {
        Ok(Vec::new())
    }

    /// Answer the requests this agent can serve. Upstream
    /// `Trader::GetMatlBids`.
    ///
    /// `commod_requests` holds every request in the step, grouped by
    /// commodity. An agent should ignore commodities it does not supply.
    ///
    /// # Errors
    ///
    /// Archetype-specific.
    fn material_bids(
        &mut self,
        _ctx: &Context,
        _me: AgentId,
        _commod_requests: &CommodRequests,
    ) -> Result<Vec<BidPortfolio>> {
        Ok(Vec::new())
    }

    /// Produce the resources for the orders this agent won. Upstream
    /// `Trader::GetMatlTrades`.
    ///
    /// Must return exactly one resource per order, in the same order.
    ///
    /// # Errors
    ///
    /// Archetype-specific — typically an inventory that cannot cover a matched
    /// order, which indicates a constraint the agent failed to declare.
    fn respond_to_trades(
        &mut self,
        _ctx: &Context,
        _orders: &[Order],
        _masses: &AtomicMasses,
    ) -> Result<Vec<Resource>> {
        Ok(Vec::new())
    }

    /// Take delivery. Upstream `Trader::AcceptMatlTrades`.
    ///
    /// # Errors
    ///
    /// Archetype-specific — typically a buffer that cannot hold the delivery,
    /// which again means an undeclared constraint.
    fn accept_trades(
        &mut self,
        _ctx: &Context,
        _deliveries: Vec<Delivery>,
        _masses: &AtomicMasses,
    ) -> Result<()> {
        Ok(())
    }

    /// Whether this agent should be decommissioned at the end of this step.
    /// Upstream `Facility::CheckDecommissionCondition`.
    fn check_decommission(&self, _ctx: &Context) -> bool {
        false
    }

    /// A one-line human-readable state summary, for diagnostics. Upstream
    /// `Agent::str`.
    fn summary(&self) -> String;
}

/// The concrete archetypes, dispatched by `match` rather than by `dyn`.
///
/// Upstream resolves these at run time from shared libraries; here they are
/// resolved at compile time, so an unknown archetype cannot reach a running
/// simulation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AgentKind {
    /// A fixed-throughput material source. See [`Source`].
    Source(Source),
    /// A fixed-throughput material sink. See [`Sink`].
    Sink(Sink),
    /// An agent with no behaviour of its own.
    ///
    /// This is what a [`Region`](crate::agent::AgentRole::Region) or
    /// [`Institution`](crate::agent::AgentRole::Institution) is until the
    /// CYCAMORE archetypes that give them behaviour — `GrowthRegion`,
    /// `DeployInst`, `ManagerInst` — are ported. It holds a place in the
    /// hierarchy and does nothing else.
    ///
    /// This is **not** a stub standing in for an unported facility. A
    /// facility that silently trades nothing would make a simulation run and
    /// produce a plausible wrong answer, which is why the unported facility
    /// archetypes are absent rather than inert. Upstream's own base `Region`
    /// and `Institution` genuinely do almost nothing beyond the hierarchy, so
    /// this variant is a faithful translation rather than a placeholder.
    Inert,
}

/// Forwards every [`Facility`] method to the active variant.
///
/// This is the whole cost of enum dispatch, and it is paid once here rather
/// than at every call site. Adding a variant makes each of these `match`
/// expressions a compile error until it is handled, which is precisely the
/// property `dyn` cannot give.
impl Facility for AgentKind {
    fn tick(&mut self, ctx: &Context) -> Result<()> {
        match self {
            Self::Source(a) => a.tick(ctx),
            Self::Sink(a) => a.tick(ctx),
            Self::Inert => Ok(()),
        }
    }

    fn tock(&mut self, ctx: &Context) -> Result<()> {
        match self {
            Self::Source(a) => a.tock(ctx),
            Self::Sink(a) => a.tock(ctx),
            Self::Inert => Ok(()),
        }
    }

    fn material_requests(&mut self, ctx: &Context, me: AgentId) -> Result<Vec<RequestPortfolio>> {
        match self {
            Self::Source(a) => a.material_requests(ctx, me),
            Self::Sink(a) => a.material_requests(ctx, me),
            Self::Inert => Ok(Vec::new()),
        }
    }

    fn material_bids(
        &mut self,
        ctx: &Context,
        me: AgentId,
        commod_requests: &CommodRequests,
    ) -> Result<Vec<BidPortfolio>> {
        match self {
            Self::Source(a) => a.material_bids(ctx, me, commod_requests),
            Self::Sink(a) => a.material_bids(ctx, me, commod_requests),
            Self::Inert => Ok(Vec::new()),
        }
    }

    fn respond_to_trades(
        &mut self,
        ctx: &Context,
        orders: &[Order],
        masses: &AtomicMasses,
    ) -> Result<Vec<Resource>> {
        match self {
            Self::Source(a) => a.respond_to_trades(ctx, orders, masses),
            Self::Sink(a) => a.respond_to_trades(ctx, orders, masses),
            Self::Inert => Ok(Vec::new()),
        }
    }

    fn accept_trades(
        &mut self,
        ctx: &Context,
        deliveries: Vec<Delivery>,
        masses: &AtomicMasses,
    ) -> Result<()> {
        match self {
            Self::Source(a) => a.accept_trades(ctx, deliveries, masses),
            Self::Sink(a) => a.accept_trades(ctx, deliveries, masses),
            Self::Inert => Ok(()),
        }
    }

    fn check_decommission(&self, ctx: &Context) -> bool {
        match self {
            Self::Source(a) => a.check_decommission(ctx),
            Self::Sink(a) => a.check_decommission(ctx),
            Self::Inert => false,
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Source(a) => a.summary(),
            Self::Sink(a) => a.summary(),
            Self::Inert => String::from("inert hierarchy agent"),
        }
    }
}

/// A complete simulation: the agent arena, the context, and the phase loop.
///
/// Upstream splits this between `Context` (which owns the agents) and `Timer`
/// (which drives them). Here [`Context`] holds the simulation-global services
/// and `Simulation` holds the agents, so an agent can be handed `&Context`
/// without the aliasing that a `Context` owning it would create.
#[derive(Debug, Clone, PartialEq)]
pub struct Simulation {
    ctx: Context,
    infos: Vec<AgentInfo>,
    agents: Vec<AgentKind>,
    masses: AtomicMasses,
    trade_log: Vec<(i64, Trade)>,
}

impl Simulation {
    /// Creates a simulation over `ctx`, converting compositions with `masses`.
    #[must_use]
    pub fn new(ctx: Context, masses: AtomicMasses) -> Self {
        Self {
            ctx,
            infos: Vec::new(),
            agents: Vec::new(),
            masses: masses.clone(),
            trade_log: Vec::new(),
        }
    }

    /// Adds an agent and returns its handle.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `parent` does not exist, or if the hierarchy
    /// nesting is wrong — a facility must sit under an institution, an
    /// institution under a region, and a region at the root. Upstream accepts
    /// any parent, so a malformed deck yields a quietly wrong hierarchy.
    pub fn add_agent(
        &mut self,
        role: AgentRole,
        prototype: &str,
        spec: &str,
        parent: Option<AgentId>,
        agent: AgentKind,
    ) -> Result<AgentId> {
        let parent_role = match parent {
            None => None,
            Some(p) => Some(
                self.infos
                    .get(p.index())
                    .ok_or(CyclusError::Value("parent agent does not exist"))?
                    .role,
            ),
        };
        if !role.may_sit_under(parent_role) {
            return Err(CyclusError::Value(
                "agent hierarchy must be Region > Institution > Facility",
            ));
        }

        let id = AgentId(self.infos.len());
        let mut info = AgentInfo::new(id, role, prototype, spec, self.ctx.time());
        if let Some(p) = parent {
            info.parent = Some(p);
            self.infos[p.index()].children.push(id);
        }
        self.infos.push(info);
        self.agents.push(agent);
        Ok(id)
    }

    /// The simulation context.
    #[must_use]
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    /// The bookkeeping for `id`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if no such agent exists.
    pub fn info(&self, id: AgentId) -> Result<&AgentInfo> {
        self.infos
            .get(id.index())
            .ok_or(CyclusError::Key("no agent with that id"))
    }

    /// The archetype state for `id`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if no such agent exists.
    pub fn agent(&self, id: AgentId) -> Result<&AgentKind> {
        self.agents
            .get(id.index())
            .ok_or(CyclusError::Key("no agent with that id"))
    }

    /// Every trade that has cleared, with the time step it cleared on.
    ///
    /// This is the translation of upstream's `Transactions` output table, kept
    /// in memory because this kernel has no database. It is what a test or a
    /// downstream analysis reads.
    #[must_use]
    pub fn trade_log(&self) -> &[(i64, Trade)] {
        &self.trade_log
    }

    /// The handles of every agent that trades this step.
    ///
    /// **Only facilities trade.** Upstream, `Trader` is implemented by
    /// `Facility` alone — a `Region` or `Institution` decides what gets
    /// built, never what gets bought and sold. Filtering here rather than
    /// relying on the hierarchy agents to return empty portfolios is what
    /// makes that structural: an institution cannot accidentally enter the
    /// market, whatever archetype it is given.
    #[must_use]
    pub fn trading_agents(&self) -> Vec<AgentId> {
        let now = self.ctx.time();
        self.infos
            .iter()
            .filter(|i| i.alive(now) && i.role == AgentRole::Facility)
            .map(|i| i.id)
            .collect()
    }

    /// The handles of every agent alive at the current time step.
    #[must_use]
    pub fn live_agents(&self) -> Vec<AgentId> {
        let now = self.ctx.time();
        self.infos
            .iter()
            .filter(|i| i.alive(now))
            .map(|i| i.id)
            .collect()
    }

    /// Runs the simulation to completion. Upstream `Timer::RunSim`.
    ///
    /// # Errors
    ///
    /// Whatever any phase reports.
    pub fn run(&mut self) -> Result<()> {
        while self.ctx.timer().running() {
            self.step()?;
            self.ctx.timer_mut().advance();
        }
        Ok(())
    }

    /// Runs one time step, all six phases in order.
    ///
    /// Does **not** advance the clock — [`run`](Self::run) does that, so a
    /// caller driving the loop itself can inspect state between the step and
    /// the tick over.
    ///
    /// # Errors
    ///
    /// Whatever any phase reports.
    pub fn step(&mut self) -> Result<()> {
        for phase in Phase::ORDER {
            match phase {
                Phase::Build => self.do_build(),
                Phase::Tick => self.do_tick()?,
                Phase::Exchange => self.do_exchange()?,
                Phase::Tock => self.do_tock()?,
                Phase::Decision => self.do_decision(),
                Phase::Decommission => self.do_decommission(),
            }
        }
        Ok(())
    }

    /// Upstream `Timer::DoBuild`.
    ///
    /// Scheduled builds are drained so they do not fire twice. Actually
    /// constructing the agent needs a prototype registry, which this crate
    /// does not have (archetypes resolve at compile time), so a caller
    /// schedules a build and adds the agent itself. The queue is drained here
    /// to keep the phase honest rather than silently unimplemented.
    fn do_build(&mut self) {
        let t = self.ctx.time();
        let _due = self.ctx.take_builds_due(t);
    }

    /// Upstream `Timer::DoTick`.
    fn do_tick(&mut self) -> Result<()> {
        for id in self.live_agents() {
            let ctx = self.ctx.clone();
            self.agents[id.index()].tick(&ctx)?;
        }
        Ok(())
    }

    /// Upstream `Timer::DoTock`.
    fn do_tock(&mut self) -> Result<()> {
        for id in self.live_agents() {
            let ctx = self.ctx.clone();
            self.agents[id.index()].tock(&ctx)?;
        }
        Ok(())
    }

    /// Upstream `Timer::DoDecision`, plus its retirement sweep.
    fn do_decision(&mut self) {
        let now = self.ctx.time();
        let mut to_decom = Vec::new();
        for id in self.live_agents() {
            if self.agents[id.index()].check_decommission(&self.ctx)
                || self.infos[id.index()].retired(now)
            {
                to_decom.push(id);
            }
        }
        for id in to_decom {
            self.ctx.sched_decom(id, now);
        }
    }

    /// Upstream `Timer::DoDecom`.
    fn do_decommission(&mut self) {
        let t = self.ctx.time();
        for d in self.ctx.take_decoms_due(t) {
            if let Some(info) = self.infos.get_mut(d.agent.index()) {
                info.decommissioned = true;
            }
        }
    }

    /// The dynamic resource exchange. Upstream `Timer::DoResEx` and
    /// `ExchangeManager<T>::Execute`.
    ///
    /// The five steps are the ones in this module's documentation: gather
    /// requests, gather bids, solve, respond, accept.
    fn do_exchange(&mut self) -> Result<()> {
        let live = self.trading_agents();
        let ctx = self.ctx.clone();
        let mut xctx = ExchangeContext::new();

        // 1. Requests.
        let mut commod_requests: CommodRequests = BTreeMap::new();
        for id in &live {
            for portfolio in self.agents[id.index()].material_requests(&ctx, *id)? {
                let pidx = xctx.add_request_portfolio(portfolio);
                let reqs = xctx.request_portfolios()[pidx].requests().to_vec();
                for (ridx, r) in reqs.into_iter().enumerate() {
                    let rid = RequestId {
                        portfolio: pidx,
                        index: ridx,
                    };
                    commod_requests
                        .entry(r.commodity().to_string())
                        .or_default()
                        .push((rid, r));
                }
            }
        }
        if commod_requests.is_empty() {
            return Ok(());
        }

        // 2. Bids.
        for id in &live {
            for portfolio in self.agents[id.index()].material_bids(&ctx, *id, &commod_requests)? {
                xctx.add_bid_portfolio(portfolio);
            }
        }

        // 3. Solve.
        let mut translation = xctx.translate()?;
        let mut solver = GreedySolver::new();
        solver.solve(&mut translation.graph)?;
        let trades = translation.back_translate()?;
        if trades.is_empty() {
            return Ok(());
        }

        // 4. Respond: group the winning orders by supplier.
        let mut by_supplier: BTreeMap<usize, Vec<Order>> = BTreeMap::new();
        for t in &trades {
            let bid = xctx.bid(t.bid)?;
            let req = xctx.request(t.request)?;
            by_supplier
                .entry(bid.bidder().index())
                .or_default()
                .push(Order {
                    trade: *t,
                    commodity: req.commodity().to_string(),
                    target: req.target().clone(),
                });
        }

        let mut deliveries: BTreeMap<usize, Vec<Delivery>> = BTreeMap::new();
        for (supplier, orders) in &by_supplier {
            let produced =
                self.agents[*supplier].respond_to_trades(&ctx, orders, &self.masses)?;
            if produced.len() != orders.len() {
                return Err(CyclusError::State(
                    "a supplier returned a different number of resources than it had orders",
                ));
            }
            for (order, resource) in orders.iter().zip(produced) {
                let req = xctx.request(order.trade.request)?;
                deliveries
                    .entry(req.requester().index())
                    .or_default()
                    .push(Delivery {
                        trade: order.trade,
                        commodity: order.commodity.clone(),
                        resource,
                    });
            }
        }

        // 5. Accept.
        for (requester, got) in deliveries {
            self.agents[requester].accept_trades(&ctx, got, &self.masses)?;
        }

        let t = self.ctx.time();
        self.trade_log.extend(trades.into_iter().map(|tr| (t, tr)));
        Ok(())
    }
}
