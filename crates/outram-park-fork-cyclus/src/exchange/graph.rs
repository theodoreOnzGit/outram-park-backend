// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/exchange_graph.h, src/exchange_graph.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! The resource-neutral exchange graph: nodes, arcs, groups and matches.
//!
//! This is the structure a [solver](crate::exchange::greedy) actually works
//! on. Requests and bids — which know about resources, agents and commodities
//! — are *translated* into it, and the solution comes back out as a list of
//! [`Match`]es which are translated back into
//! [`Trade`](crate::exchange::trade::Trade)s. Nothing in this module knows
//! what a [`Material`](crate::material::Material) is; that is the point.
//!
//! # Shape of the graph
//!
//! It is bipartite. By upstream's convention the **u** side is the requesting
//! side and the **v** side is the bidding side, so for every [`Arc`],
//! `arc.unode()` is a request node and `arc.vnode()` is a bid node.
//!
//! Nodes are collected into [`ExchangeNodeGroup`]s. A *request* group is one
//! requester's portfolio and carries the total quantity that portfolio wants;
//! a *supply* group is one bidder's portfolio. Each group carries a vector of
//! **capacities**, and each node carries, per arc, a vector of **unit
//! capacities** — how much of each group capacity one unit of flow along that
//! arc consumes. The solver divides one by the other to get the flow a
//! constraint permits.
//!
//! # Arena indices instead of `shared_ptr`
//!
//! Upstream's graph is an object graph of `boost::shared_ptr<ExchangeNode>`
//! with `weak_ptr` back-references on each arc, and a `std::map` keyed on the
//! pointers. This workspace forbids trait objects, `Box` and lifetimes, so the
//! whole thing becomes an **arena**: [`ExchangeGraph`] owns flat `Vec`s of
//! nodes, arcs and groups, and every cross-reference is a [`NodeId`],
//! [`ArcId`] or [`GroupId`] index into them.
//!
//! Three things improve as a result, and they are worth stating because they
//! are not merely a workaround:
//!
//! 1. **The cycle disappears.** Upstream needs `weak_ptr` on `Arc` precisely
//!    because nodes own arcs which reference nodes. An index has no ownership,
//!    so there is nothing to break.
//! 2. **Iteration order becomes deterministic.** `std::map<Arc, double>` keyed
//!    on an `Arc` whose `operator<` compares *pointer values* iterates in an
//!    order that depends on the allocator. Here the same maps are keyed on
//!    [`ArcId`], which is insertion order, so two runs of the same input sum
//!    preferences in the same order and produce bit-identical results.
//! 3. **`arc_ids_` and `arc_by_id_` vanish.** Upstream keeps two maps just to
//!    give each arc an integer id; here the id *is* the index.
//!
//! # Inheritance becomes a tag
//!
//! Upstream's `RequestGroup` derives from `ExchangeNodeGroup` and adds a
//! `qty_`. There is one struct here, [`ExchangeNodeGroup`], carrying a
//! [`GroupKind`] and a quantity that only request groups use. Both group
//! vectors on the graph are then lists of [`GroupId`] into a single arena,
//! which is what lets [`ExchangeNode::group`] be one plain index — a
//! `GroupId` that had to say *which* of two arenas it indexed would be an
//! enum, and every capacity lookup would have to match on it.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{CyclusError, Result};
use crate::exchange::request::DEFAULT_PREF;
use crate::limits::{abs, float_distance, FLOAT_ULP_EQ, UNLIMITED};

/// An index into an [`ExchangeGraph`]'s node arena.
///
/// Replaces upstream's `boost::shared_ptr<ExchangeNode>`. Ordering is
/// insertion order, which makes every map keyed on a node deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// An index into an [`ExchangeGraph`]'s arc arena.
///
/// Replaces upstream's `Arc` used as a `std::map` key. Ordering is insertion
/// order; upstream ordered by the two node *pointers*, which is why its
/// preference maps iterate unpredictably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArcId(pub usize);

/// An index into an [`ExchangeGraph`]'s group arena.
///
/// Both request and supply groups live in one arena, so a `GroupId` alone
/// identifies a group; [`ExchangeNodeGroup::kind`] says which sort it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(pub usize);

/// Which side of the exchange a group sits on.
///
/// Upstream expresses this with inheritance — `RequestGroup : public
/// ExchangeNodeGroup` — which cannot survive the no-trait-objects rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupKind {
    /// A requester's portfolio. Upstream `RequestGroup`. Carries a total
    /// requested quantity, and automatically places each exclusive node in an
    /// exclusive group of its own.
    Request,
    /// A bidder's portfolio. Upstream a plain `ExchangeNodeGroup`.
    Supply,
}

/// A translated request or bid.
///
/// One node is one [`Request`](crate::exchange::request::Request) or one
/// [`Bid`](crate::exchange::bid::Bid), stripped of everything the solver does
/// not need. Upstream `cyclus::ExchangeNode`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeNode {
    /// The group this node belongs to, set by
    /// [`ExchangeGraph::add_node`]. `None` only for a node that has not been
    /// added to a graph; the solver rejects such a node, because a capacity
    /// has no meaning without a group to draw it from.
    pub group: Option<GroupId>,

    /// Per-arc unit capacities: how much of each of the parent group's
    /// capacities one unit of flow along that arc consumes.
    ///
    /// The inner `Vec` is positionally aligned with
    /// [`ExchangeNodeGroup::capacities`] — entry `i` is the unit consumption
    /// of group capacity `i`. Dimensionless: it is
    /// `converter(offer) / offer.quantity()`, so for the default
    /// quantity-converter every entry is `1.0`.
    ///
    /// An arc absent from this map, or present with an empty vector, means
    /// "unconstrained along this arc" and the node's own [`qty`](Self::qty) is
    /// the only limit. Upstream gets that behaviour from `std::map::operator[]`
    /// default-inserting an empty vector.
    pub unit_capacities: BTreeMap<ArcId, Vec<f64>>,

    /// Per-arc preference, as seen by the *requesting* node. Strictly
    /// positive; larger is more preferred.
    ///
    /// Only request nodes carry entries here. Upstream reads it with
    /// `std::map::operator[]`, which silently yields `0` for an arc that was
    /// never assigned a preference; that is reproduced by treating a missing
    /// key as `0.0`, and it matters because the objective divides by it.
    pub prefs: BTreeMap<ArcId, f64>,

    /// Whether this request or bid must be satisfied in full or not at all.
    pub exclusive: bool,

    /// The commodity name, used by the
    /// [preconditioner](crate::exchange::preconditioner) to look up a weight.
    pub commod: String,

    /// The id of the agent this node belongs to. Upstream's default is `-1`,
    /// meaning "unset"; it is used only to break preference ties, so that the
    /// ordering is total and therefore reproducible.
    pub agent_id: i32,

    /// The maximum quantity that may be assigned to this node — the requested
    /// amount for a request node, the offered amount for a bid node. Units are
    /// the resource's (kilograms for a material).
    pub qty: f64,
}

impl Default for ExchangeNode {
    /// An unconstrained node: quantity [`UNLIMITED`], not exclusive, no
    /// commodity, agent `-1`. Upstream's no-argument `ExchangeNode()`.
    fn default() -> Self {
        Self {
            group: None,
            unit_capacities: BTreeMap::new(),
            prefs: BTreeMap::new(),
            exclusive: false,
            commod: String::new(),
            agent_id: -1,
            qty: UNLIMITED,
        }
    }
}

impl ExchangeNode {
    /// A node for `qty` units of a resource, not exclusive.
    ///
    /// `qty` is in the resource's own units and must be non-negative; nothing
    /// enforces that here because upstream does not either, but a negative
    /// quantity will simply never be matched (the solver requires a match
    /// larger than [`EPS`](crate::limits::EPS)).
    ///
    /// Upstream's four `ExchangeNode` constructors become this plus the
    /// builder methods below.
    #[must_use]
    pub fn new(qty: f64) -> Self {
        Self {
            qty,
            ..Self::default()
        }
    }

    /// Marks the node exclusive (all-or-nothing) or not.
    #[must_use]
    pub fn exclusive(mut self, exclusive: bool) -> Self {
        self.exclusive = exclusive;
        self
    }

    /// Sets the commodity name.
    #[must_use]
    pub fn commodity(mut self, commod: &str) -> Self {
        self.commod = commod.to_string();
        self
    }

    /// Sets the owning agent's id.
    #[must_use]
    pub fn agent(mut self, agent_id: i32) -> Self {
        self.agent_id = agent_id;
        self
    }

    /// The preference of the arc `a` as seen from this node, or `0.0` if this
    /// node has no preference recorded for it.
    ///
    /// The zero default is upstream's, and is deliberately *not* softened to
    /// `1.0`: the objective term is `qty / pref`, so a missing preference
    /// blows the objective up to infinity rather than quietly scoring the
    /// trade as if it were ordinary. A translated graph always has a
    /// preference on every arc.
    #[must_use]
    pub fn pref(&self, a: ArcId) -> f64 {
        self.prefs.get(&a).copied().unwrap_or(0.0)
    }
}

/// A possible trade: a connection from a request node to a bid node.
///
/// Upstream `cyclus::Arc`. Copyable here, because it is five scalars once the
/// `weak_ptr`s become indices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    unode: NodeId,
    vnode: NodeId,
    exclusive: bool,
    excl_val: f64,
    pref: f64,
}

impl Arc {
    /// The request node. Upstream `Arc::unode()`.
    #[must_use]
    pub fn unode(&self) -> NodeId {
        self.unode
    }

    /// The bid node. Upstream `Arc::vnode()`.
    #[must_use]
    pub fn vnode(&self) -> NodeId {
        self.vnode
    }

    /// Whether either endpoint is exclusive, so that flow along this arc must
    /// be all-or-nothing. Upstream `Arc::exclusive()`.
    #[must_use]
    pub fn exclusive(&self) -> bool {
        self.exclusive
    }

    /// The only quantity that may flow along this arc if it is exclusive, in
    /// the resource's units; `0.0` for a non-exclusive arc, and also `0.0` for
    /// an exclusive arc whose endpoints disagree about the quantity (such an
    /// arc can never carry flow). See [`exclusive_value`].
    #[must_use]
    pub fn excl_val(&self) -> f64 {
        self.excl_val
    }

    /// The requester's preference for this arc. Strictly positive; larger is
    /// better. Upstream `Arc::pref()`.
    #[must_use]
    pub fn pref(&self) -> f64 {
        self.pref
    }
}

/// The quantity an exclusive arc must carry, or `0.0` if it can carry none.
///
/// This is upstream's `Arc::Arc(unode, vnode)` body, lifted out so it can be
/// tested directly, and it is one of the subtlest few lines in Cyclus.
///
/// # Why the ULP comparison
///
/// An exclusive order is all-or-nothing, so an arc between an exclusive
/// request for `x` and a bid of `y` can only ever carry flow if `x` and `y`
/// are the *same* quantity. But both numbers have usually been through a
/// division and a multiplication by the time they get here, so an exact `==`
/// would reject arcs that are equal in every sense that matters. Upstream
/// therefore compares them with `boost::math::float_distance` and accepts a
/// disagreement of up to [`FLOAT_ULP_EQ`] (2) representable values, with the
/// comment that this "is vital for preventing false positive constraint
/// violations w.r.t. exclusivity-related capacity". The translation uses
/// [`float_distance`](crate::limits::float_distance), which is that function.
///
/// # The three cases
///
/// With `dist = float_distance(u_qty, v_qty)` (positive when the bid offers
/// more than the request wants):
///
/// * **both exclusive** — the quantities must agree to within 2 ULP in either
///   direction, and the arc carries the request's quantity.
/// * **only the request is exclusive** — the bid must offer *at least* the
///   requested amount (`dist >= -2` ULP), and the arc carries the requested
///   amount. A larger bid is fine; it is simply not fully consumed.
/// * **only the bid is exclusive** — the request must want *at least* the
///   offered amount (`dist <= +2` ULP), and the arc carries the offered
///   amount.
///
/// # Parameters
///
/// `u_qty`/`v_qty` are the request-side and bid-side quantities in the
/// resource's units; `u_excl`/`v_excl` say which side is exclusive.
#[must_use]
pub fn exclusive_value(u_qty: f64, u_excl: bool, v_qty: f64, v_excl: bool) -> f64 {
    if !(u_excl || v_excl) {
        return 0.0;
    }
    let dist = float_distance(u_qty, v_qty);
    if u_excl && v_excl {
        if abs(dist) <= FLOAT_ULP_EQ {
            u_qty
        } else {
            0.0
        }
    } else if u_excl {
        if dist >= -FLOAT_ULP_EQ {
            u_qty
        } else {
            0.0
        }
    } else if dist <= FLOAT_ULP_EQ {
        v_qty
    } else {
        0.0
    }
}

/// One requester's or one bidder's portfolio, as the solver sees it.
///
/// Upstream `cyclus::ExchangeNodeGroup` and its subclass `RequestGroup`,
/// merged into one struct tagged with a [`GroupKind`] (see the module doc).
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeNodeGroup {
    kind: GroupKind,
    qty: f64,
    nodes: Vec<NodeId>,
    excl_node_groups: Vec<Vec<NodeId>>,
    capacities: Vec<f64>,
}

impl ExchangeNodeGroup {
    /// Which side of the exchange this group is on.
    #[must_use]
    pub fn kind(&self) -> GroupKind {
        self.kind
    }

    /// The total quantity this group wants, in the resource's units.
    ///
    /// Meaningful only for [`GroupKind::Request`]; a supply group reports
    /// `0.0`. Upstream `RequestGroup::qty()`. It is *not* the sum of the
    /// member nodes' quantities when the portfolio declared mutual requests —
    /// see [`RequestPortfolio::add_mutual_reqs`](crate::exchange::portfolio::RequestPortfolio::add_mutual_reqs).
    #[must_use]
    pub fn qty(&self) -> f64 {
        self.qty
    }

    /// The nodes in this group, in solver order.
    ///
    /// The order is meaningful and mutable: the
    /// [preconditioner](crate::exchange::preconditioner) and the greedy solver
    /// both re-sort it, and the solver walks it front to back.
    #[must_use]
    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    /// Sets of nodes over which flow may exist on at most one arc.
    ///
    /// Upstream `excl_node_groups()`. Populated for bid portfolios where
    /// several bids offer the same physical resource, and automatically for
    /// every exclusive node of a request group.
    ///
    /// **The greedy solver does not read this.** It is consumed only by
    /// upstream's Coin-OR/Cbc `ProgTranslator`, which is out of scope for this
    /// crate; it is ported so a future solver has it and so the translation of
    /// `RequestGroup::AddExchangeNode` stays faithful.
    #[must_use]
    pub fn excl_node_groups(&self) -> &[Vec<NodeId>] {
        &self.excl_node_groups
    }

    /// The group's flow capacities, one per
    /// [`CapacityConstraint`](crate::exchange::constraint::CapacityConstraint)
    /// on the originating portfolio, in the same order.
    ///
    /// Units are the constraint's, not necessarily the resource's — an
    /// enrichment SWU constraint is in SWU. The node-side
    /// [`unit_capacities`](ExchangeNode::unit_capacities) carry the conversion.
    /// A capacity of exactly [`UNLIMITED`] means unbounded; see
    /// [`GreedySolver`](crate::exchange::greedy::GreedySolver).
    #[must_use]
    pub fn capacities(&self) -> &[f64] {
        &self.capacities
    }

    /// `true` if any node in this group has at least one arc. Upstream
    /// `ExchangeNodeGroup::HasArcs`, which likewise tests the preference map.
    #[must_use]
    pub fn has_arcs(&self, graph: &ExchangeGraph) -> bool {
        self.nodes
            .iter()
            .any(|n| !graph.nodes[n.0].prefs.is_empty())
    }

    pub(crate) fn nodes_mut(&mut self) -> &mut Vec<NodeId> {
        &mut self.nodes
    }
}

/// A solved flow: `qty` units along `arc`.
///
/// Upstream `typedef std::pair<Arc, double> Match`. A named struct here
/// because `.0`/`.1` on a pair of an arc and a quantity reads badly at every
/// call site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Match {
    /// The arc carrying the flow.
    pub arc: ArcId,
    /// How much flows, in the resource's units. Strictly positive.
    pub qty: f64,
}

/// The whole exchange, as a graph.
///
/// Built by translating portfolios (see
/// [`ExchangeContext`](crate::exchange::translate::ExchangeContext)), solved
/// by a [`GreedySolver`](crate::exchange::greedy::GreedySolver), and read back
/// as [`Match`]es. Upstream `cyclus::ExchangeGraph`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExchangeGraph {
    nodes: Vec<ExchangeNode>,
    node_arcs: Vec<Vec<ArcId>>,
    arcs: Vec<Arc>,
    groups: Vec<ExchangeNodeGroup>,
    request_groups: Vec<GroupId>,
    supply_groups: Vec<GroupId>,
    matches: Vec<Match>,
}

impl ExchangeGraph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a request group wanting `qty` units in total, and returns its id.
    ///
    /// Upstream `AddRequestGroup` plus `RequestGroup::RequestGroup(qty)`.
    pub fn add_request_group(&mut self, qty: f64) -> GroupId {
        let id = GroupId(self.groups.len());
        self.groups.push(ExchangeNodeGroup {
            kind: GroupKind::Request,
            qty,
            nodes: Vec::new(),
            excl_node_groups: Vec::new(),
            capacities: Vec::new(),
        });
        self.request_groups.push(id);
        id
    }

    /// Adds a supply (bid) group and returns its id. Upstream
    /// `AddSupplyGroup`.
    pub fn add_supply_group(&mut self) -> GroupId {
        let id = GroupId(self.groups.len());
        self.groups.push(ExchangeNodeGroup {
            kind: GroupKind::Supply,
            qty: 0.0,
            nodes: Vec::new(),
            excl_node_groups: Vec::new(),
            capacities: Vec::new(),
        });
        self.supply_groups.push(id);
        id
    }

    /// Adds `node` to `group` and returns its id.
    ///
    /// Upstream `ExchangeNodeGroup::AddExchangeNode`, including
    /// `RequestGroup`'s override: an exclusive node added to a **request**
    /// group is additionally placed in an exclusive group of its own.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `group` is not a group of this graph.
    pub fn add_node(&mut self, group: GroupId, node: ExchangeNode) -> Result<NodeId> {
        let kind = self
            .groups
            .get(group.0)
            .ok_or(CyclusError::Key("no such exchange node group"))?
            .kind;
        let exclusive = node.exclusive;
        let id = NodeId(self.nodes.len());
        let mut node = node;
        node.group = Some(group);
        self.nodes.push(node);
        self.node_arcs.push(Vec::new());

        let g = &mut self.groups[group.0];
        g.nodes.push(id);
        if kind == GroupKind::Request && exclusive {
            g.excl_node_groups.push(vec![id]);
        }
        Ok(id)
    }

    /// Records that flow may exist over at most one of `nodes`' arcs.
    /// Upstream `AddExclGroup`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `group` is not a group of this graph.
    pub fn add_excl_group(&mut self, group: GroupId, nodes: Vec<NodeId>) -> Result<()> {
        self.groups
            .get_mut(group.0)
            .ok_or(CyclusError::Key("no such exchange node group"))?
            .excl_node_groups
            .push(nodes);
        Ok(())
    }

    /// Appends a flow capacity to `group`. Upstream `AddCapacity`.
    ///
    /// Capacities are positional: the `i`-th capacity here pairs with the
    /// `i`-th entry of every member node's
    /// [`unit_capacities`](ExchangeNode::unit_capacities), so add them in the
    /// same order the unit capacities are pushed.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `group` is not a group of this graph.
    pub fn add_capacity(&mut self, group: GroupId, capacity: f64) -> Result<()> {
        self.groups
            .get_mut(group.0)
            .ok_or(CyclusError::Key("no such exchange node group"))?
            .capacities
            .push(capacity);
        Ok(())
    }

    /// Connects request node `unode` to bid node `vnode` and returns the new
    /// arc's id.
    ///
    /// The arc's exclusivity and [`excl_val`](Arc::excl_val) are computed here
    /// from the two nodes, exactly as upstream's `Arc` constructor does — see
    /// [`exclusive_value`]. The preference starts at
    /// [`DEFAULT_PREF`](crate::exchange::request::DEFAULT_PREF); set the real
    /// one with [`set_arc_pref`](Self::set_arc_pref).
    ///
    /// Upstream leaves `Arc::pref_` **uninitialised** in this constructor and
    /// relies on the translator to assign it before anything reads it. Starting
    /// from unity instead costs nothing and removes a way to get garbage out of
    /// a hand-built graph.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if either node is not a node of this graph.
    pub fn add_arc(&mut self, unode: NodeId, vnode: NodeId) -> Result<ArcId> {
        let u = self
            .nodes
            .get(unode.0)
            .ok_or(CyclusError::Key("no such exchange node"))?;
        let v = self
            .nodes
            .get(vnode.0)
            .ok_or(CyclusError::Key("no such exchange node"))?;
        let exclusive = u.exclusive || v.exclusive;
        let excl_val = exclusive_value(u.qty, u.exclusive, v.qty, v.exclusive);

        let id = ArcId(self.arcs.len());
        self.arcs.push(Arc {
            unode,
            vnode,
            exclusive,
            excl_val,
            pref: DEFAULT_PREF,
        });
        self.node_arcs[unode.0].push(id);
        self.node_arcs[vnode.0].push(id);
        Ok(id)
    }

    /// Sets an arc's preference. Upstream `Arc::pref(double)`.
    ///
    /// Must be strictly positive: the objective divides by it. This does not
    /// also write the request node's
    /// [`prefs`](ExchangeNode::prefs) entry — the translator does both,
    /// because upstream keeps the two copies too (the arc's is read by the
    /// pseudo-cost, the node's by the solver's ordering and objective).
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `arc` is not an arc of this graph.
    pub fn set_arc_pref(&mut self, arc: ArcId, pref: f64) -> Result<()> {
        self.arcs
            .get_mut(arc.0)
            .ok_or(CyclusError::Key("no such arc"))?
            .pref = pref;
        Ok(())
    }

    /// Records `qty` units of flow along `arc`. Upstream `AddMatch`.
    pub fn add_match(&mut self, arc: ArcId, qty: f64) {
        self.matches.push(Match { arc, qty });
    }

    /// Discards every recorded match, so the graph can be re-solved.
    /// Upstream `ClearMatches`.
    pub fn clear_matches(&mut self) {
        self.matches.clear();
    }

    /// The solution: every flow the solver assigned, in the order it assigned
    /// them.
    #[must_use]
    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// Every node, indexed by [`NodeId`].
    #[must_use]
    pub fn nodes(&self) -> &[ExchangeNode] {
        &self.nodes
    }

    /// Every arc, indexed by [`ArcId`].
    #[must_use]
    pub fn arcs(&self) -> &[Arc] {
        &self.arcs
    }

    /// Every group, indexed by [`GroupId`], request and supply intermixed in
    /// creation order.
    #[must_use]
    pub fn groups(&self) -> &[ExchangeNodeGroup] {
        &self.groups
    }

    /// The request groups, in the order the solver will visit them. The
    /// [preconditioner](crate::exchange::preconditioner) permutes this.
    #[must_use]
    pub fn request_groups(&self) -> &[GroupId] {
        &self.request_groups
    }

    /// The supply groups, in creation order.
    #[must_use]
    pub fn supply_groups(&self) -> &[GroupId] {
        &self.supply_groups
    }

    /// The arcs incident on a node. Upstream `node_arc_map()`.
    ///
    /// For a request node these are the bids answering it; for a bid node,
    /// the requests it answers. Returns an empty slice for an unknown node —
    /// upstream's own solver has a special case for exactly this, since
    /// `std::map::at` would throw for a request that attracted no bids.
    #[must_use]
    pub fn arcs_for_node(&self, node: NodeId) -> &[ArcId] {
        self.node_arcs
            .get(node.0)
            .map_or(&[][..], |v| v.as_slice())
    }

    /// Borrows a node.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `node` is not a node of this graph.
    pub fn node(&self, node: NodeId) -> Result<&ExchangeNode> {
        self.nodes
            .get(node.0)
            .ok_or(CyclusError::Key("no such exchange node"))
    }

    /// Mutably borrows a node, to set its preferences or unit capacities.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `node` is not a node of this graph.
    pub fn node_mut(&mut self, node: NodeId) -> Result<&mut ExchangeNode> {
        self.nodes
            .get_mut(node.0)
            .ok_or(CyclusError::Key("no such exchange node"))
    }

    /// Borrows an arc.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `arc` is not an arc of this graph.
    pub fn arc(&self, arc: ArcId) -> Result<&Arc> {
        self.arcs
            .get(arc.0)
            .ok_or(CyclusError::Key("no such arc"))
    }

    /// Borrows a group.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if `group` is not a group of this graph.
    pub fn group(&self, group: GroupId) -> Result<&ExchangeNodeGroup> {
        self.groups
            .get(group.0)
            .ok_or(CyclusError::Key("no such exchange node group"))
    }

    pub(crate) fn group_mut(&mut self, group: GroupId) -> Result<&mut ExchangeNodeGroup> {
        self.groups
            .get_mut(group.0)
            .ok_or(CyclusError::Key("no such exchange node group"))
    }

    pub(crate) fn request_groups_mut(&mut self) -> &mut Vec<GroupId> {
        &mut self.request_groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// `x` moved `n` representable values up.
    fn up(x: f64, n: u64) -> f64 {
        f64::from_bits(x.to_bits() + n)
    }

    /// `x` moved `n` representable values down (positive `x` only).
    fn down(x: f64, n: u64) -> f64 {
        f64::from_bits(x.to_bits() - n)
    }

    #[test]
    fn non_exclusive_arcs_have_no_exclusive_value() {
        assert_eq!(exclusive_value(10.0, false, 4.0, false), 0.0);
    }

    #[test]
    fn two_exclusive_nodes_agree_within_two_ulps_but_not_three() {
        // 1 ULP apart in either direction: the arc carries the request's qty.
        assert_eq!(exclusive_value(4.0, true, up(4.0, 1), true), 4.0);
        assert_eq!(exclusive_value(4.0, true, down(4.0, 1), true), 4.0);
        // Exactly at the 2-ULP threshold, still accepted (`<=`).
        assert_eq!(exclusive_value(4.0, true, up(4.0, 2), true), 4.0);
        // 3 ULP apart: the arc can never carry flow.
        assert_eq!(exclusive_value(4.0, true, up(4.0, 3), true), 0.0);
        assert_eq!(exclusive_value(4.0, true, down(4.0, 3), true), 0.0);
    }

    #[test]
    fn an_exclusive_request_needs_a_bid_at_least_as_large() {
        // Bid strictly larger: fine, and only the requested amount flows.
        assert_eq!(exclusive_value(4.0, true, 10.0, false), 4.0);
        // Bid 1 ULP short: inside tolerance, still accepted.
        assert_eq!(exclusive_value(4.0, true, down(4.0, 1), false), 4.0);
        // Bid 3 ULP short: rejected.
        assert_eq!(exclusive_value(4.0, true, down(4.0, 3), false), 0.0);
    }

    #[test]
    fn an_exclusive_bid_needs_a_request_at_least_as_large() {
        // Request strictly larger: fine, and the whole offer flows.
        assert_eq!(exclusive_value(10.0, false, 4.0, true), 4.0);
        // Request 1 ULP short of the offer: inside tolerance.
        assert_eq!(exclusive_value(down(4.0, 1), false, 4.0, true), 4.0);
        // Request 3 ULP short: rejected.
        assert_eq!(exclusive_value(down(4.0, 3), false, 4.0, true), 0.0);
    }

    #[test]
    fn add_arc_wires_both_node_arc_lists_and_derives_exclusivity() {
        let mut g = ExchangeGraph::new();
        let rg = g.add_request_group(10.0);
        let sg = g.add_supply_group();
        let u = g.add_node(rg, ExchangeNode::new(10.0)).unwrap();
        let v = g
            .add_node(sg, ExchangeNode::new(4.0).exclusive(true))
            .unwrap();
        let a = g.add_arc(u, v).unwrap();

        assert_eq!(g.arcs_for_node(u), &[a]);
        assert_eq!(g.arcs_for_node(v), &[a]);
        assert!(g.arc(a).unwrap().exclusive());
        assert_eq!(g.arc(a).unwrap().excl_val(), 4.0);
        assert_eq!(g.arc(a).unwrap().pref(), DEFAULT_PREF);
        assert_eq!(g.arc(a).unwrap().unode(), u);
        assert_eq!(g.arc(a).unwrap().vnode(), v);
    }

    #[test]
    fn a_request_group_auto_groups_its_exclusive_nodes_and_a_supply_group_does_not() {
        let mut g = ExchangeGraph::new();
        let rg = g.add_request_group(4.0);
        let sg = g.add_supply_group();
        let u = g
            .add_node(rg, ExchangeNode::new(4.0).exclusive(true))
            .unwrap();
        g.add_node(sg, ExchangeNode::new(4.0).exclusive(true))
            .unwrap();

        assert_eq!(g.group(rg).unwrap().excl_node_groups(), &[vec![u]]);
        assert!(g.group(sg).unwrap().excl_node_groups().is_empty());
    }

    #[test]
    fn nodes_learn_their_group_and_groups_report_their_kind() {
        let mut g = ExchangeGraph::new();
        let rg = g.add_request_group(7.5);
        let n = g.add_node(rg, ExchangeNode::new(7.5)).unwrap();
        assert_eq!(g.node(n).unwrap().group, Some(rg));
        assert_eq!(g.group(rg).unwrap().kind(), GroupKind::Request);
        assert_eq!(g.group(rg).unwrap().qty(), 7.5);
        assert_eq!(g.request_groups(), &[rg]);
        assert!(g.supply_groups().is_empty());
    }

    #[test]
    fn has_arcs_follows_the_preference_map_as_upstream_does() {
        let mut g = ExchangeGraph::new();
        let rg = g.add_request_group(1.0);
        let sg = g.add_supply_group();
        let u = g.add_node(rg, ExchangeNode::new(1.0)).unwrap();
        let v = g.add_node(sg, ExchangeNode::new(1.0)).unwrap();
        let a = g.add_arc(u, v).unwrap();
        // The arc exists, but upstream's HasArcs tests `prefs`, not the arc
        // list, so it is still false until a preference is assigned.
        assert!(!g.group(rg).unwrap().has_arcs(&g));
        g.node_mut(u).unwrap().prefs.insert(a, 1.0);
        assert!(g.group(rg).unwrap().has_arcs(&g));
    }

    #[test]
    fn a_default_node_is_unlimited_and_a_missing_preference_reads_as_zero() {
        let n = ExchangeNode::default();
        assert_eq!(n.qty, UNLIMITED);
        assert_eq!(n.agent_id, -1);
        assert!(!n.exclusive);
        assert_eq!(n.pref(ArcId(0)), 0.0);
    }

    #[test]
    fn unknown_ids_are_key_errors_not_panics() {
        let g = ExchangeGraph::new();
        assert_eq!(
            g.node(NodeId(0)).unwrap_err(),
            CyclusError::Key("no such exchange node")
        );
        assert_eq!(g.arcs_for_node(NodeId(9)), &[] as &[ArcId]);
    }
}
