// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/exchange_graph.{h,cc}, src/greedy_solver.{h,cc},
//                     src/greedy_preconditioner.{h,cc}, src/exchange_solver.{h,cc},
//                     src/request.h, src/bid.h, src/request_portfolio.h,
//                     src/bid_portfolio.h, src/capacity_constraint.h,
//                     src/trade.h, src/exchange_translator.h,
//                     src/exchange_context.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! **Dynamic resource exchange** — how a Cyclus time step decides who sends
//! what to whom.
//!
//! This is the idea that distinguishes Cyclus from every fuel-cycle code that
//! came before it, and it is worth reading before anything else in this crate.
//! If you understand this module you understand Cyclus.
//!
//! # The problem
//!
//! A fuel cycle is a set of facilities — mines, conversion plants, enrichment
//! plants, fabricators, reactors, reprocessing plants, repositories — that
//! pass material between one another. The obvious way to simulate that is to
//! wire them together: this mine feeds that conversion plant, which feeds that
//! enricher. Every fuel-cycle code before Cyclus did roughly this.
//!
//! The trouble is that the interesting questions are exactly the ones that
//! break the wiring. What happens if a reprocessing plant is unavailable for
//! six months? If a new reactor type is introduced in 2045? If two utilities
//! compete for the same enrichment capacity? Answering any of those means
//! rewiring, and a model that must be rewired to ask a question is a model
//! that answers only the questions its author already had.
//!
//! # The idea
//!
//! Cyclus does not wire facilities together at all. **Every time step, it
//! holds a market.** The market is recomputed from scratch each step, so who
//! trades with whom is an *output* of the simulation rather than an input.
//!
//! The market runs in five phases.
//!
//! ## 1. Requests
//!
//! Every facility that needs something posts a [`Request`](request::Request):
//! a commodity name, a target [`Resource`](crate::resource::Resource) saying
//! how much is wanted (and, for a material, of what composition), and a
//! **preference** saying how keenly it is wanted. A reactor posts a request
//! for `"uox"`; a repository posts one for `"waste"`.
//!
//! Requests are grouped into a [`RequestPortfolio`](portfolio::RequestPortfolio) —
//! one per facility per commodity family. A portfolio is the unit of *mutual*
//! satisfaction: putting two requests in one portfolio and declaring them
//! mutual (see
//! [`add_mutual_reqs`](portfolio::RequestPortfolio::add_mutual_reqs)) says
//! "either of these will do", which is how a reactor says it will take MOX or
//! UOX but does not need both.
//!
//! ## 2. Bids
//!
//! Every facility that can supply something looks at the posted requests and
//! answers the ones it can fill with a [`Bid`](bid::Bid): *this* request, and
//! *this* resource in reply. A bid may offer less than was asked for, or more.
//! A bidder may answer many requests, and many bidders may answer one request.
//! Bids are grouped into a [`BidPortfolio`](portfolio::BidPortfolio), one per
//! bidding facility.
//!
//! ## 3. Constraints
//!
//! Neither side can honour everything it posted. An enrichment plant that bids
//! on six requests cannot fill all six — it has a fixed monthly separative
//! work capacity, and a fixed natural-uranium inventory. So each portfolio
//! carries [`CapacityConstraint`](constraint::CapacityConstraint)s, which act
//! across *all* of its requests or bids at once.
//!
//! A constraint is a **budget** plus an **exchange rate**. The budget is a
//! number: 100 SWU, 4 tonnes of throughput, 50 kg of inventory. The exchange
//! rate is a [`Converter`](constraint::Converter), which says how much of that
//! budget one unit of a particular resource would consume — trivially the
//! resource's own mass, but for an enricher, the separative work that
//! *specific* product would take. Dividing budget by rate turns "100 SWU"
//! into "therefore at most 43 kg of this product", which is a number the
//! solver can work with.
//!
//! ## 4. Solving
//!
//! The portfolios are then **translated** (see [`translate`]) into an
//! [`ExchangeGraph`](graph::ExchangeGraph): a bipartite graph whose left-hand
//! nodes are requests, whose right-hand nodes are bids, and whose arcs are
//! possible trades. Everything resource-specific is stripped out in the
//! process — a node is a quantity, an exclusivity flag and a list of unit
//! capacities, nothing more — so the solver works on pure numbers and the same
//! solver handles materials and products alike.
//!
//! The [`GreedySolver`](greedy::GreedySolver) then walks the graph: requests
//! in order of how keenly they want their bids, and within each request, bids
//! in order of preference, assigning as much flow as the constraints allow
//! until demand is met or supply runs out. It is greedy — nothing is ever
//! revisited — which is why the ordering done by the
//! [`GreedyPreconditioner`](preconditioner::GreedyPreconditioner) is as
//! important as the matching itself.
//!
//! ## 5. Trades
//!
//! The solution comes back as a list of [`Match`](graph::Match)es on the
//! graph, which [`Translation::back_translate`](translate::Translation::back_translate)
//! turns into [`Trade`](trade::Trade)s: this request, this bid, this much.
//! The agents then actually move the material, and the time step ends.
//!
//! # Preferences, in one paragraph
//!
//! A preference is a strictly positive dimensionless number attached to a
//! (request, bid) pair; larger means more wanted. It does two jobs. It
//! **orders** the greedy walk, so a keenly preferred bid is offered the flow
//! first. And it **prices** the solution: the solver minimises
//! `sum(quantity / preference)`, so a preference behaves as the reciprocal of
//! a unit cost. Unfilled demand is charged a pseudo-cost constructed to exceed
//! any real arc's cost, which is what makes two solutions that fill different
//! amounts of demand comparable at all.
//!
//! A requester sets the preference on its own request. A bidder may override
//! it — but only on the requester's behalf, by evaluating a cost function the
//! requester published. A bidder that simply raises its own preference is
//! rigging the market.
//!
//! # Exclusive orders
//!
//! Some things cannot be split. A reactor's fuel assembly is one assembly or
//! none; half of one is worthless. Either side may declare itself
//! **exclusive**, and an arc touching an exclusive node carries either its
//! whole quantity or nothing at all. This is the subtlest part of the
//! subsystem, it is arithmetically delicate, and it is documented at
//! [`exclusive_value`](graph::exclusive_value) and in the
//! [`greedy`] module doc.
//!
//! # A worked example
//!
//! ```
//! use kaki_bukit::agent::AgentId;
//! use kaki_bukit::exchange::constraint::CapacityConstraint;
//! use kaki_bukit::exchange::greedy::GreedySolver;
//! use kaki_bukit::exchange::portfolio::{BidPortfolio, RequestPortfolio};
//! use kaki_bukit::exchange::request::{RequestId, DEFAULT_PREF};
//! use kaki_bukit::exchange::translate::ExchangeContext;
//! use kaki_bukit::product::Product;
//! use kaki_bukit::resource::Resource;
//!
//! # fn main() -> Result<(), kaki_bukit::error::CyclusError> {
//! let power = |qty| Resource::from(Product::new(qty, "MWh").unwrap());
//!
//! // A city wants 100 MWh.
//! let mut ctx = ExchangeContext::new();
//! let mut demand = RequestPortfolio::new();
//! demand.request(power(100.0), AgentId(1), "power", DEFAULT_PREF, false)?;
//! ctx.add_request_portfolio(demand);
//! let the_request = RequestId { portfolio: 0, index: 0 };
//!
//! // A cheap plant bids the lot but can only deliver 40 MWh this step...
//! let mut cheap = BidPortfolio::new();
//! cheap.bid(the_request, power(100.0), AgentId(2), false)?;
//! cheap.add_constraint(CapacityConstraint::new(40.0)?);
//! ctx.add_bid_portfolio(cheap);
//!
//! // ...and a peaker bids 80 MWh, less preferred.
//! let mut peaker = BidPortfolio::new();
//! peaker.add_bid(
//!     kaki_bukit::exchange::bid::Bid::new(
//!         the_request, power(80.0), AgentId(3), false,
//!     )?
//!     .with_preference(0.5)?,
//! )?;
//! ctx.add_bid_portfolio(peaker);
//!
//! let mut xlate = ctx.translate()?;
//! let mut solver = GreedySolver::new();
//! solver.solve(&mut xlate.graph)?;
//!
//! // The cheap plant supplies its 40 MWh first; the peaker covers the rest.
//! let trades = xlate.back_translate()?;
//! assert_eq!(trades.len(), 2);
//! assert_eq!(trades[0].amt, 40.0);
//! assert_eq!(trades[1].amt, 60.0);
//! assert_eq!(solver.unmatched(), 0.0);
//! # Ok(())
//! # }
//! ```
//!
//! # How this translation differs from upstream
//!
//! The substitutions are uniform and are each argued for where they occur;
//! this table is the index.
//!
//! | Upstream | Here | Where it is argued |
//! |---|---|---|
//! | `shared_ptr`/`weak_ptr` object graph | arena indices: [`NodeId`](graph::NodeId), [`ArcId`](graph::ArcId), [`GroupId`](graph::GroupId) | [`graph`] |
//! | `RequestGroup : ExchangeNodeGroup` | one struct with a [`GroupKind`](graph::GroupKind) tag | [`graph`] |
//! | `Converter<T>`, an abstract base class | [`Converter`](constraint::Converter), an enum | [`constraint`] |
//! | `Request<T>` / `Bid<T>` templates | non-generic, holding a [`Resource`](crate::resource::Resource) enum | [`request`] |
//! | `int agent_id` with `-1` for unset | `Option<AgentId>` | [`graph`] |
//! | bid preference as a quiet `NaN` sentinel | `Option<f64>` | [`bid`] |
//! | offer identity by pointer | an explicit [`shared offer tag`](bid::Bid::with_shared_offer) | [`bid`] |
//! | thrown exceptions | [`Result`](crate::error::Result) | [`crate::error`] |
//! | `ProgSolver` on Coin-OR/Cbc | **not ported** | [`greedy`] |
//!
//! Two upstream behaviours are preserved *exactly* even though they look like
//! defects, because they are not: exact float equality against
//! [`UNLIMITED`](crate::limits::UNLIMITED) as an unbounded-capacity sentinel,
//! and ULP-distance comparison for exclusive orders. Both are explained in the
//! [`greedy`] module doc. Do not "fix" either.

pub mod bid;
pub mod constraint;
pub mod graph;
pub mod greedy;
pub mod portfolio;
pub mod preconditioner;
pub mod request;
pub mod trade;
pub mod translate;

pub use bid::{Bid, BidId};
pub use constraint::{CapacityConstraint, Converter};
pub use graph::{
    exclusive_value, Arc, ArcId, ExchangeGraph, ExchangeNode, ExchangeNodeGroup, GroupId,
    GroupKind, Match, NodeId,
};
pub use greedy::GreedySolver;
pub use portfolio::{BidPortfolio, RequestPortfolio};
pub use preconditioner::{GreedyPreconditioner, WgtOrder};
pub use request::{Request, RequestId, DEFAULT_PREF};
pub use trade::Trade;
pub use translate::{ExchangeContext, Translation};
