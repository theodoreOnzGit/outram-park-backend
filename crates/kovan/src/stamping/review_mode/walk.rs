//! **A review walk** (GitHub #770; #740 decisions of 2026-10-06 and U2,
//! U4, decision 10): the functions under a target, bottom-up, with the
//! walk's size before it starts, a huge-walk warning, the trail taken and
//! the end-of-walk summary.
//!
//! ```text
//!   target ──callees (kovan.toml, workspace only)──> reachable set
//!      │  Tarjan SCCs: callees before callers; mutual recursion = 1 group
//!      v
//!   WalkPlan { groups bottom-up, each item's state now }  ── WalkSize, huge_warning
//!      │  [Next ▸]: the first group not done whose outside callees are valid
//!      v
//!   Trail (stamped | needs fix | skipped), then WalkSummary + target state
//! ```
//!
//! **Rules, as decided:**
//! - bottom-up is enforced: a function cannot be stamped until every
//!   workspace function it calls has a valid stamp ([`blockers`]); calls
//!   into std, external crates and unresolved calls are not in `kovan.toml`
//!   callees, so they never block; mutually recursive functions are one
//!   group and do not block each other;
//! - a function already valid is shown with a tick and skipped;
//! - the walk's size (function count, lines, needs-fix entries) is shown
//!   before it starts; a huge walk gets a warning and the choice stays with
//!   the maintainer: kovan never picks a smaller target ([`huge_warning`]);
//! - [Next ▸] opens the next function that is now unblocked; the
//!   maintainer may wander off and come back (the trail is kept);
//! - the summary counts stamped, needs-fix and skipped, plus the target's
//!   new state.
//!
//! Pure: the graph and the states come in, nothing is read here. The
//! loader ([`super::plan_walk`]) builds them from the workspace.

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::review::state::StateKind;

/// A walk with more functions to review than this is "huge" (a proposal:
/// about a day of reviewing at a few minutes each; recorded for the
/// maintainer in the #770 report).
pub const HUGE_FUNCTIONS: usize = 60;
/// A walk with more lines to read than this is "huge" (same proposal).
pub const HUGE_LINES: u32 = 3000;

/// One function of the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkItem {
    /// The stable `fn:` id.
    pub id: String,
    /// The call-graph id (kovan-web's key, for navigation).
    pub call_graph_id: String,
    /// `Type::name`, for display.
    pub name: String,
    /// Lines (doc comment included).
    pub lines: u32,
    /// Its state now.
    pub state: StateKind,
    /// The workspace callees (`fn:` ids).
    pub callees: Vec<String>,
}

impl WalkItem {
    /// Valid now (ticked and skipped by the walk).
    pub fn valid(&self) -> bool {
        self.state.counts()
    }
}

/// Bottom-up groups over the functions reachable from `target` through
/// `callees` (Tarjan's strongly connected components, which come out
/// callees first). Each group is sorted; a group of more than one is a
/// mutual recursion. Ids not in `callees` are leaves.
pub fn bottom_up(target: &str, callees: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    // Iterative Tarjan (no recursion depth limit on long call chains).
    let empty = Vec::new();
    let succ = |v: &str| callees.get(v).unwrap_or(&empty);
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut low: BTreeMap<String, usize> = BTreeMap::new();
    let mut on_stack: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = Vec::new();
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut next = 0usize;
    // (node, next successor position)
    let mut work: Vec<(String, usize)> = vec![(target.to_string(), 0)];
    while let Some((v, i)) = work.pop() {
        if i == 0 && !index.contains_key(&v) {
            index.insert(v.clone(), next);
            low.insert(v.clone(), next);
            next += 1;
            stack.push(v.clone());
            on_stack.insert(v.clone());
        }
        let s = succ(&v);
        if let Some(w) = s.get(i) {
            work.push((v.clone(), i + 1));
            if !index.contains_key(w) {
                work.push((w.clone(), 0));
            } else if on_stack.contains(w) {
                let lw = index[w];
                let lv = low[&v];
                low.insert(v.clone(), lv.min(lw));
            }
            continue;
        }
        // All successors done: the frame below is the parent that pushed
        // `v`; propagate `v`'s low link to it.
        if let Some((parent, _)) = work.last() {
            let (lv, lp) = (low[&v], low[parent]);
            low.insert(parent.clone(), lp.min(lv));
        }
        if low[&v] == index[&v] {
            let mut group = Vec::new();
            while let Some(w) = stack.pop() {
                on_stack.remove(&w);
                let done = w == v;
                group.push(w);
                if done {
                    break;
                }
            }
            group.sort();
            out.push(group);
        }
    }
    out
}

/// The callees of `id` that block its stamp (module doc): workspace callees
/// whose state does not count, minus the members of its own recursion
/// group and itself.
pub fn blockers(
    id: &str,
    callees: &BTreeMap<String, Vec<String>>,
    counts: impl Fn(&str) -> bool,
) -> Vec<String> {
    let group: BTreeSet<String> = bottom_up(id, callees)
        .into_iter()
        .find(|g| g.iter().any(|x| x == id))
        .unwrap_or_default()
        .into_iter()
        .collect();
    let mut out: Vec<String> = callees
        .get(id)
        .into_iter()
        .flatten()
        .filter(|c| !group.contains(*c) && !counts(c))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

/// What a walk will ask of the reviewer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalkSize {
    /// Functions in the walk.
    pub total: usize,
    /// Of those, valid already (ticked, skipped).
    pub already_valid: usize,
    /// Functions to review (not valid).
    pub functions: usize,
    /// Their lines.
    pub lines: u32,
    /// Of those, with an open needs-fix.
    pub needs_fix: usize,
}

/// A planned walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkPlan {
    /// The target's `fn:` id.
    pub target: String,
    /// Groups, bottom-up (callees first).
    pub groups: Vec<Vec<WalkItem>>,
}

impl WalkPlan {
    /// Plan the walk to `target` over `items` (every function, by `fn:` id).
    /// `None` when the target is not among them.
    pub fn new(target: &str, items: &BTreeMap<String, WalkItem>) -> Option<WalkPlan> {
        items.get(target)?;
        let callees: BTreeMap<String, Vec<String>> = items
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.callees
                        .iter()
                        .filter(|c| items.contains_key(*c))
                        .cloned()
                        .collect(),
                )
            })
            .collect();
        let groups = bottom_up(target, &callees)
            .into_iter()
            .map(|g| g.iter().filter_map(|id| items.get(id).cloned()).collect())
            .collect();
        Some(WalkPlan {
            target: target.to_string(),
            groups,
        })
    }

    /// Every item, bottom-up.
    pub fn items(&self) -> impl Iterator<Item = &WalkItem> {
        self.groups.iter().flatten()
    }

    /// The walk's size (module doc).
    pub fn size(&self) -> WalkSize {
        let mut s = WalkSize::default();
        for i in self.items() {
            s.total += 1;
            if i.valid() {
                s.already_valid += 1;
                continue;
            }
            s.functions += 1;
            s.lines += i.lines;
            if matches!(i.state, StateKind::NeedsFixOpen) {
                s.needs_fix += 1;
            }
        }
        s
    }

    /// The target item.
    pub fn target_item(&self) -> Option<&WalkItem> {
        self.items().find(|i| i.id == self.target)
    }
}

/// The warning for a huge walk, or `None` (module doc). Plain words; the
/// walk is never shrunk.
pub fn huge_warning(s: &WalkSize) -> Option<String> {
    (s.functions > HUGE_FUNCTIONS || s.lines > HUGE_LINES).then(|| {
        format!(
            "This is a huge walk: {} functions and {} lines to review (more than {HUGE_FUNCTIONS} \
             functions or {HUGE_LINES} lines). kovan will not pick a smaller target for you; \
             you may start it, or choose a function lower down yourself.",
            s.functions, s.lines
        )
    })
}

/// What happened to a function on the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Stamped,
    NeedsFix,
    Skipped,
}

impl StepOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stamped => "stamped",
            Self::NeedsFix => "needs fix",
            Self::Skipped => "skipped",
        }
    }
}

/// The path taken (#740 decision 10: a breadcrumb on the map).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trail {
    /// (`fn:` id, name, outcome), in order; a function appears once, with
    /// its latest outcome.
    pub steps: Vec<(String, String, StepOutcome)>,
}

impl Trail {
    /// Record `outcome` for `id` (replacing an earlier one).
    pub fn record(&mut self, id: &str, name: &str, outcome: StepOutcome) {
        self.steps.retain(|(i, _, _)| i != id);
        self.steps.push((id.to_string(), name.to_string(), outcome));
    }

    pub fn outcome(&self, id: &str) -> Option<StepOutcome> {
        self.steps
            .iter()
            .find(|(i, _, _)| i == id)
            .map(|(_, _, o)| *o)
    }

    /// The breadcrumb text: `leaf ✓ › twice ⛔ › …`.
    pub fn breadcrumb(&self) -> String {
        self.steps
            .iter()
            .map(|(_, n, o)| {
                let mark = match o {
                    StepOutcome::Stamped => "\u{2713}",
                    StepOutcome::NeedsFix => "\u{26d4}",
                    StepOutcome::Skipped => "\u{21b7}",
                };
                format!("{n} {mark}")
            })
            .collect::<Vec<_>>()
            .join(" \u{203a} ")
    }
}

/// Where [Next ▸] goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Review this function (a `WalkItem`).
    Review(WalkItem),
    /// Functions remain, but each waits on a callee that is not valid
    /// (skipped or marked needs fix on this walk): `(function, its blockers)`.
    Stuck(Vec<(String, Vec<String>)>),
    /// Nothing left: show the summary.
    Done,
}

/// The next function to review (module doc): the first group, bottom-up,
/// with a member that is neither valid nor on the trail, whose callees
/// outside the group are valid (now, or stamped on this walk).
pub fn next(plan: &WalkPlan, trail: &Trail) -> Next {
    let ok: BTreeSet<&str> = plan
        .items()
        .filter(|i| i.valid() || trail.outcome(&i.id) == Some(StepOutcome::Stamped))
        .map(|i| i.id.as_str())
        .collect();
    let in_walk: BTreeSet<&str> = plan.items().map(|i| i.id.as_str()).collect();
    let mut stuck = Vec::new();
    for g in &plan.groups {
        let members: BTreeSet<&str> = g.iter().map(|i| i.id.as_str()).collect();
        for i in g {
            if ok.contains(i.id.as_str()) || trail.outcome(&i.id).is_some() {
                continue;
            }
            let blocked: Vec<String> = i
                .callees
                .iter()
                .filter(|c| {
                    in_walk.contains(c.as_str())
                        && !members.contains(c.as_str())
                        && !ok.contains(c.as_str())
                })
                .cloned()
                .collect();
            if blocked.is_empty() {
                return Next::Review(i.clone());
            }
            stuck.push((i.id.clone(), blocked));
        }
    }
    if stuck.is_empty() {
        Next::Done
    } else {
        Next::Stuck(stuck)
    }
}

/// The end-of-walk summary (#740 U4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkSummary {
    pub stamped: usize,
    pub needs_fix: usize,
    pub skipped: usize,
    /// The target's state now (from the last plan).
    pub target_state: Option<StateKind>,
}

impl WalkSummary {
    pub fn new(plan: &WalkPlan, trail: &Trail) -> WalkSummary {
        let count = |o| trail.steps.iter().filter(|s| s.2 == o).count();
        WalkSummary {
            stamped: count(StepOutcome::Stamped),
            needs_fix: count(StepOutcome::NeedsFix),
            skipped: count(StepOutcome::Skipped),
            target_state: plan.target_item().map(|i| i.state),
        }
    }

    /// One line for the summary card.
    pub fn text(&self) -> String {
        format!(
            "{} stamped \u{b7} {} needs fix \u{b7} {} skipped \u{b7} target now: {}",
            self.stamped,
            self.needs_fix,
            self.skipped,
            self.target_state.map(|s| s.label()).unwrap_or("not known")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(edges: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        edges
            .iter()
            .map(|(a, bs)| (a.to_string(), bs.iter().map(|b| b.to_string()).collect()))
            .collect()
    }

    fn item(id: &str, callees: &[&str], state: StateKind, lines: u32) -> WalkItem {
        WalkItem {
            id: id.into(),
            call_graph_id: format!("a.rs::{id}"),
            name: id.into(),
            lines,
            state,
            callees: callees.iter().map(|c| c.to_string()).collect(),
        }
    }

    /// Methodology: a diamond (t -> a, b; a -> c; b -> c), a mutual
    /// recursion (c <-> d) and self-recursion (b -> b). Pass: callees come
    /// before callers; c and d form one group; unreachable e is absent.
    #[test]
    fn bottom_up_orders_callees_first_and_groups_recursion() {
        let c = g(&[
            ("t", &["a", "b"]),
            ("a", &["c"]),
            ("b", &["c", "b"]),
            ("c", &["d"]),
            ("d", &["c"]),
            ("e", &["t"]),
        ]);
        let order = bottom_up("t", &c);
        assert_eq!(order[0], vec!["c".to_string(), "d".to_string()]);
        let pos = |x: &str| {
            order
                .iter()
                .position(|grp| grp.iter().any(|y| y == x))
                .unwrap()
        };
        assert!(
            pos("a") > pos("c")
                && pos("b") > pos("c")
                && pos("t") > pos("a")
                && pos("t") > pos("b")
        );
        assert_eq!(order.len(), 4);
        assert!(order.iter().all(|grp| !grp.contains(&"e".to_string())));
        // A long chain does not overflow the stack.
        let long: BTreeMap<String, Vec<String>> = (0..20_000)
            .map(|i| (format!("f{i}"), vec![format!("f{}", i + 1)]))
            .collect();
        assert_eq!(bottom_up("f0", &long).len(), 20_001);
    }

    /// Methodology: the enforced bottom-up rule. Pass: an invalid callee
    /// blocks; a valid one does not; a mutual-recursion partner and the
    /// function itself never block.
    #[test]
    fn blockers_follow_the_bottom_up_rule() {
        let c = g(&[("t", &["a", "b", "t"]), ("a", &["t"]), ("b", &[])]);
        let valid = |x: &str| x == "b";
        assert!(
            blockers("t", &c, valid).is_empty(),
            "a is t's recursion partner"
        );
        let c2 = g(&[("t", &["a", "b"]), ("a", &[]), ("b", &[])]);
        assert_eq!(blockers("t", &c2, valid), vec!["a".to_string()]);
        assert!(blockers("b", &c2, valid).is_empty());
    }

    /// Methodology: a walk of four functions (one valid, one with an open
    /// needs-fix), then a walk past the thresholds. Pass: the size counts
    /// only what is to review; the warning appears only for the huge walk.
    #[test]
    fn size_and_huge_warning() {
        let items: BTreeMap<String, WalkItem> = [
            item("t", &["a", "b"], StateKind::New, 30),
            item("a", &["c"], StateKind::NeedsFixOpen, 20),
            item("b", &[], StateKind::Valid, 10),
            item("c", &[], StateKind::DirectlyStale, 5),
        ]
        .into_iter()
        .map(|i| (i.id.clone(), i))
        .collect();
        let p = WalkPlan::new("t", &items).unwrap();
        assert_eq!(
            p.size(),
            WalkSize {
                total: 4,
                already_valid: 1,
                functions: 3,
                lines: 55,
                needs_fix: 1
            }
        );
        assert_eq!(huge_warning(&p.size()), None);
        assert!(huge_warning(&WalkSize {
            functions: HUGE_FUNCTIONS + 1,
            ..WalkSize::default()
        })
        .is_some());
        assert!(huge_warning(&WalkSize {
            lines: HUGE_LINES + 1,
            ..WalkSize::default()
        })
        .unwrap()
        .contains("smaller target"));
        assert!(WalkPlan::new("zz", &items).is_none());
    }

    /// Methodology: walk t -> a -> c, b valid. Stamp c, mark a needs fix.
    /// Pass: Next goes c, then a, then is stuck on t (blocked by a); after
    /// skipping nothing is left; the summary counts 1/1/0 and the breadcrumb
    /// lists the trail.
    #[test]
    fn next_trail_and_summary() {
        let items: BTreeMap<String, WalkItem> = [
            item("t", &["a", "b"], StateKind::New, 30),
            item("a", &["c"], StateKind::New, 20),
            item("b", &[], StateKind::Valid, 10),
            item("c", &[], StateKind::New, 5),
        ]
        .into_iter()
        .map(|i| (i.id.clone(), i))
        .collect();
        let p = WalkPlan::new("t", &items).unwrap();
        let mut trail = Trail::default();
        let Next::Review(i) = next(&p, &trail) else {
            panic!()
        };
        assert_eq!(i.id, "c");
        trail.record("c", "c", StepOutcome::Stamped);
        let Next::Review(i) = next(&p, &trail) else {
            panic!()
        };
        assert_eq!(i.id, "a");
        trail.record("a", "a", StepOutcome::NeedsFix);
        assert_eq!(
            next(&p, &trail),
            Next::Stuck(vec![("t".into(), vec!["a".into()])])
        );
        trail.record("t", "t", StepOutcome::Skipped);
        assert_eq!(next(&p, &trail), Next::Done);
        let s = WalkSummary::new(&p, &trail);
        assert_eq!((s.stamped, s.needs_fix, s.skipped), (1, 1, 1));
        assert_eq!(s.target_state, Some(StateKind::New));
        assert!(s.text().starts_with("1 stamped"));
        assert_eq!(
            trail.breadcrumb(),
            "c \u{2713} \u{203a} a \u{26d4} \u{203a} t \u{21b7}"
        );
        trail.record("c", "c", StepOutcome::Skipped);
        assert_eq!(trail.steps.len(), 3, "one entry per function");
    }
}
