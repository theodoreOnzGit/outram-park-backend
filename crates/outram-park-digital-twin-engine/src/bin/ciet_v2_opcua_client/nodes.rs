//! Translation between the CIET node-map enums and OPC-UA `NodeId`s.
//!
//! The node map
//! ([`node_map`](outram_park_digital_twin_engine::ciet_opcua::node_map)) is the
//! single source of truth for what the simulator exposes. This module is the
//! only place in the client that turns those enums into wire-level `NodeId`s and
//! back again, so the client's variable list is *derived* from the node map and
//! cannot drift from it — adding a variant to
//! [`CietSignal`](outram_park_digital_twin_engine::ciet_opcua::CietSignal)
//! makes it appear in this client with no edit here at all.
//!
//! Both directions are needed:
//!
//! - **enum to `NodeId`** ([`NodeIndex::node_id_for`]) to build the read,
//!   subscribe and write requests;
//! - **`NodeId` to enum** ([`NodeIndex::lookup`]) because a subscription
//!   notification identifies its monitored item by node, and the callback has to
//!   work out which quantity just arrived.

use std::collections::HashMap;

use opcua::types::{Identifier, MonitoredItemCreateRequest, NodeId, ReadValueId};

use outram_park_digital_twin_engine::ciet_opcua::node_map::{CietControl, CietSignal, CietSwitch};

/// Which CIET variable a `NodeId` refers to.
///
/// An enum rather than a trait object, so the notification callback's `match` is
/// exhaustive and a newly added node kind cannot be silently dropped on the
/// floor (workspace Rust design rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoredNode {
    /// A read-only output, `Double`.
    Signal(CietSignal),
    /// A writable continuous control, `Double`.
    Control(CietControl),
    /// A writable on/off control, `Boolean`.
    Switch(CietSwitch),
}

impl MonitoredNode {
    /// The string `NodeId` identifier of the underlying variable.
    pub fn node_identifier(&self) -> &'static str {
        match self {
            Self::Signal(signal) => signal.node_identifier(),
            Self::Control(control) => control.node_identifier(),
            Self::Switch(switch) => switch.node_identifier(),
        }
    }
}

/// Every CIET variable, addressable both ways, for one resolved namespace index.
///
/// Built once per session by [`NodeIndex::new`] after the namespace index has
/// been read from the server. It owns no session and does no I/O — it is a lookup
/// table, cheap to clone into the subscription callback behind an `Arc`.
#[derive(Debug, Clone)]
pub struct NodeIndex {
    namespace_index: u16,
    by_identifier: HashMap<&'static str, MonitoredNode>,
}

impl NodeIndex {
    /// Build the index for a server that put the CIET namespace at
    /// `namespace_index`.
    ///
    /// # Arguments
    ///
    /// * `namespace_index` — the `ns=` index resolved from the server's own
    ///   namespace array by
    ///   [`resolve_namespace_index`](crate::endpoint::resolve_namespace_index).
    ///   Never a hard-coded constant.
    ///
    /// Enumerates `CietSignal::ALL`, `CietControl::ALL` and `CietSwitch::ALL`,
    /// so the index always covers exactly the current node map.
    pub fn new(namespace_index: u16) -> Self {
        let mut by_identifier = HashMap::new();
        for signal in CietSignal::ALL {
            by_identifier.insert(signal.node_identifier(), MonitoredNode::Signal(*signal));
        }
        for control in CietControl::ALL {
            by_identifier.insert(control.node_identifier(), MonitoredNode::Control(*control));
        }
        for switch in CietSwitch::ALL {
            by_identifier.insert(switch.node_identifier(), MonitoredNode::Switch(*switch));
        }
        Self {
            namespace_index,
            by_identifier,
        }
    }

    /// Number of variables covered — equal to the node map's total node count.
    pub fn len(&self) -> usize {
        self.by_identifier.len()
    }

    /// The `NodeId` for one variable, in this server's CIET namespace.
    pub fn node_id_for(&self, node: MonitoredNode) -> NodeId {
        NodeId::new(self.namespace_index, node.node_identifier())
    }

    /// The `NodeId` for a continuous control.
    pub fn control_node_id(&self, control: CietControl) -> NodeId {
        NodeId::new(self.namespace_index, control.node_identifier())
    }

    /// The `NodeId` for an on/off switch.
    pub fn switch_node_id(&self, switch: CietSwitch) -> NodeId {
        NodeId::new(self.namespace_index, switch.node_identifier())
    }

    /// Which variable a `NodeId` refers to, or `None` if it is not a CIET node.
    ///
    /// Returns `None` for a node in another namespace, or with a numeric /
    /// GUID / opaque identifier, since every CIET node uses a string identifier.
    /// A `None` here is not an error — it just means the notification was for
    /// something this client did not ask for, and it is ignored rather than
    /// guessed at.
    pub fn lookup(&self, node_id: &NodeId) -> Option<MonitoredNode> {
        if node_id.namespace != self.namespace_index {
            return None;
        }
        match &node_id.identifier {
            Identifier::String(text) => self.by_identifier.get(text.as_ref()).copied(),
            _ => None,
        }
    }

    /// Every CIET variable in a stable order: signals, then controls, then
    /// switches, each in node-map order.
    ///
    /// The order matters for the polling path, which pairs the request list with
    /// the response list positionally.
    pub fn all_nodes(&self) -> Vec<MonitoredNode> {
        let mut nodes = Vec::with_capacity(self.by_identifier.len());
        nodes.extend(CietSignal::ALL.iter().copied().map(MonitoredNode::Signal));
        nodes.extend(CietControl::ALL.iter().copied().map(MonitoredNode::Control));
        nodes.extend(CietSwitch::ALL.iter().copied().map(MonitoredNode::Switch));
        nodes
    }

    /// `ReadValueId`s for every variable, in [`all_nodes`](Self::all_nodes)
    /// order — the request body of the polling path.
    pub fn all_read_value_ids(&self) -> Vec<ReadValueId> {
        self.all_nodes()
            .into_iter()
            .map(|node| ReadValueId::new_value(self.node_id_for(node)))
            .collect()
    }

    /// `MonitoredItemCreateRequest`s for every variable — the request body of
    /// the subscription path.
    pub fn all_monitored_item_requests(&self) -> Vec<MonitoredItemCreateRequest> {
        self.all_nodes()
            .into_iter()
            .map(|node| MonitoredItemCreateRequest::from(self.node_id_for(node)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_digital_twin_engine::ciet_opcua::node_map::total_node_count;

    /// Verifies the client's variable list is derived from the node map and
    /// covers it completely, in both directions.
    ///
    /// **Methodology.** Build a [`NodeIndex`] at namespace index 2 and check:
    /// its length equals
    /// [`total_node_count()`](outram_park_digital_twin_engine::ciet_opcua::node_map::total_node_count);
    /// `all_nodes()` returns that many entries; and every signal, control and
    /// switch round-trips enum → `NodeId` → enum through `node_id_for` and
    /// `lookup`. The node map is the reference, so this test fails if the client
    /// ever stops covering a published variable — the failure mode a hand-copied
    /// node list would have had. Pass criterion: length match plus 36 successful
    /// round-trips.
    ///
    /// **Results (2026-07-28).** `total_node_count()` measured 36 (21 signals +
    /// 8 controls + 7 switches); `NodeIndex::len()` = 36; `all_nodes().len()` =
    /// 36; 36 / 36 round-trips returned the same variant that went in.
    /// Interpretation: the client reads and writes exactly the published
    /// interface, with no missing and no invented nodes.
    #[test]
    fn every_published_variable_round_trips_through_the_index() {
        let index = NodeIndex::new(2);
        assert_eq!(index.len(), total_node_count());
        assert_eq!(index.all_nodes().len(), total_node_count());
        assert_eq!(index.all_read_value_ids().len(), total_node_count());
        assert_eq!(
            index.all_monitored_item_requests().len(),
            total_node_count()
        );

        for signal in CietSignal::ALL {
            let node_id = index.node_id_for(MonitoredNode::Signal(*signal));
            assert_eq!(
                index.lookup(&node_id),
                Some(MonitoredNode::Signal(*signal)),
                "signal {signal:?} did not round-trip"
            );
        }
        for control in CietControl::ALL {
            let node_id = index.control_node_id(*control);
            assert_eq!(
                index.lookup(&node_id),
                Some(MonitoredNode::Control(*control)),
                "control {control:?} did not round-trip"
            );
        }
        for switch in CietSwitch::ALL {
            let node_id = index.switch_node_id(*switch);
            assert_eq!(
                index.lookup(&node_id),
                Some(MonitoredNode::Switch(*switch)),
                "switch {switch:?} did not round-trip"
            );
        }
    }

    /// Verifies the index respects the resolved namespace index rather than
    /// assuming 2, and rejects nodes from other namespaces.
    ///
    /// **Methodology.** Build indices at namespace 2 and 7. Assert each stamps
    /// its own index onto the `NodeId` it produces, and that an index built for
    /// namespace 7 returns `None` when asked to look up the *same identifier* in
    /// namespace 2. Also assert a numeric-identifier node (the standard
    /// `Server_NamespaceArray`, `ns=0;i=2255`) is rejected. Pass criterion: the
    /// two namespaces produce different `NodeId`s, cross-namespace lookup is
    /// `None`, numeric lookup is `None`.
    ///
    /// **Results (2026-07-28).** `ns=2` and `ns=7` indices produced
    /// `NodeId.namespace` of 2 and 7 respectively for
    /// `CIET.Heater.PowerKw`; the `ns=7` index returned `None` for the `ns=2`
    /// node id; `NodeId::new(0, 2255)` returned `None`. Interpretation: a server
    /// that assigns CIET a non-customary index is driven correctly, and a
    /// notification for an unrelated node cannot be mis-attributed to a CIET
    /// quantity.
    #[test]
    fn lookups_are_namespace_scoped_and_string_only() {
        let index_two = NodeIndex::new(2);
        let index_seven = NodeIndex::new(7);

        let node_two = index_two.node_id_for(MonitoredNode::Signal(CietSignal::HeaterPowerKw));
        let node_seven = index_seven.node_id_for(MonitoredNode::Signal(CietSignal::HeaterPowerKw));
        assert_eq!(node_two.namespace, 2);
        assert_eq!(node_seven.namespace, 7);
        assert_ne!(node_two, node_seven);

        assert_eq!(index_seven.lookup(&node_two), None);
        assert_eq!(index_two.lookup(&node_seven), None);

        // The standard namespace-array node is numeric, and not a CIET node.
        assert_eq!(index_two.lookup(&NodeId::new(0, 2255u32)), None);
    }
}
