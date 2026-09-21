// ---------------------------------------------------------------------------
// The CONNECTIVE TAXONOMY below is taken from SCRAM (a probabilistic risk
// analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/event.h (enum Connective), src/pdag.h (enum Connective)
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: [`Connective`] mirrors upstream's eight-variant `pdag.h`
// enum, with `kAtleast`'s separate `min_number` field folded into the variant
// as `Atleast { min }` so an at-least gate cannot exist without its threshold.
// Upstream's three MEF-only connectives (`kIff`, `kImply`, `kCardinality`,
// declared in `event.h` but not in `pdag.h`) are NOT included: the
// preprocessor rewrites them away before any analysis, and that preprocessor
// is not ported.
//
// The REST of this file is not a port. Upstream's tree lives in `Model` /
// `FaultTree` / `Formula` objects built by an XML `Initializer`, and is
// then rewritten into a `Pdag` by a 2,400-line preprocessor before MOCUS ever
// sees it. None of that is ported. This is a plain indexed structure a caller
// builds directly in Rust, since input-file parsing is out of RAFFLES' scope.
// ---------------------------------------------------------------------------

//! The fault tree itself — gates, their logic, and what feeds them.
//!
//! A fault tree is a Boolean expression written upside down: the **top event**
//! is the system failure being studied, and it is decomposed through gates
//! until the leaves are **basic events** whose probabilities are known.
//!
//! Build one with [`FaultTreeBuilder`], which works in names and hands back
//! both the tree and the basic-event probability vector that
//! [`super::probability`] and [`super::importance`] expect — the indices line
//! up by construction, which is the mistake that would otherwise be easiest to
//! make.
//!
//! ```
//! use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
//!
//! // Two redundant trains, each of which fails if its valve or its pump does.
//! let mut b = FaultTreeBuilder::new();
//! b.basic_event("ValveOne", 0.5).unwrap();
//! b.basic_event("PumpOne", 0.7).unwrap();
//! b.basic_event("ValveTwo", 0.5).unwrap();
//! b.basic_event("PumpTwo", 0.7).unwrap();
//! b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
//! b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
//! b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
//! let model = b.build("TopEvent").unwrap();
//!
//! assert_eq!(model.probabilities(), &[0.5, 0.7, 0.5, 0.7]);
//! ```

use std::collections::HashMap;

use crate::{RafflesError, Result};

/// The Boolean logic a gate applies to its arguments.
///
/// Mirrors upstream SCRAM's `pdag.h` `Connective` enum. **Only the coherent
/// subset is supported by [`super::mocus`]** — the four negating variants are
/// representable so that a tree containing one can be built and *refused with
/// a clear message*, rather than being silently unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connective {
    /// All arguments must occur. Upstream `kAnd`.
    And,
    /// Any one argument suffices. Upstream `kOr`.
    Or,
    /// At least `min` of the arguments must occur — the K-of-N, voting or
    /// combination gate. Upstream `kAtleast`, whose threshold lives in a
    /// separate `min_number` field; folding it into the variant makes an
    /// at-least gate without a threshold unrepresentable.
    ///
    /// `min` must satisfy `1 <= min <= args.len()`. `min == 1` is an
    /// [`Connective::Or`] and `min == args.len()` is an [`Connective::And`];
    /// both are accepted rather than rewritten, because upstream accepts them
    /// and rewriting would make a round trip lossy.
    Atleast {
        /// How many arguments must occur.
        min: usize,
    },
    /// Exactly one of two arguments. Upstream `kXor`. **Non-coherent.**
    Xor,
    /// Negation of a single argument. Upstream `kNot`. **Non-coherent.**
    Not,
    /// Negation of [`Connective::And`]. Upstream `kNand`. **Non-coherent.**
    Nand,
    /// Negation of [`Connective::Or`]. Upstream `kNor`. **Non-coherent.**
    Nor,
    /// Pass-through of a single argument, with no logic. Upstream `kNull`.
    ///
    /// Not the empty set — it exists because the Model Exchange Format lets a
    /// gate stand for another event (a "transfer" symbol), and dropping it
    /// would change the tree's shape.
    Null,
}

impl Connective {
    /// Whether this connective is **coherent** — monotone, so that a basic
    /// event occurring can never make the top event less likely.
    ///
    /// Non-coherent trees need complement elimination, which is not ported;
    /// [`super::mocus::minimal_cut_sets`] refuses them.
    pub fn is_coherent(&self) -> bool {
        matches!(
            self,
            Connective::And | Connective::Or | Connective::Atleast { .. } | Connective::Null
        )
    }

    /// Upstream's own name for this connective, as it appears in a SCRAM input
    /// model and in `kConnectiveToString`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Connective::And => "and",
            Connective::Or => "or",
            Connective::Atleast { .. } => "atleast",
            Connective::Xor => "xor",
            Connective::Not => "not",
            Connective::Nand => "nand",
            Connective::Nor => "nor",
            Connective::Null => "null",
        }
    }
}

/// What feeds a gate: another gate, or a basic event.
///
/// Both carry an index, not a name — gate indices into [`FaultTree::gates`],
/// basic-event indices into the probability slice. [`FaultTreeBuilder`] does
/// the name resolution so a caller need not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    /// An intermediate gate, by index.
    Gate(usize),
    /// A basic event, by index into the probability slice.
    BasicEvent(usize),
}

/// One gate: a connective and the arguments it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    connective: Connective,
    args: Vec<Arg>,
}

impl Gate {
    /// The Boolean logic this gate applies.
    pub fn connective(&self) -> Connective {
        self.connective
    }

    /// What feeds this gate, in the order it was declared.
    pub fn args(&self) -> &[Arg] {
        &self.args
    }
}

/// A validated fault tree.
///
/// Constructing one guarantees, once and for all, that every index is in
/// range, every gate's arity suits its connective, and the graph is acyclic.
/// [`super::mocus`] therefore does not re-check any of that and cannot loop
/// forever on a cyclic tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaultTree {
    gates: Vec<Gate>,
    basic_event_count: usize,
    top: usize,
}

impl FaultTree {
    /// Every gate, indexed as [`Arg::Gate`] refers to them.
    pub fn gates(&self) -> &[Gate] {
        &self.gates
    }

    /// How many distinct basic events the tree refers to.
    pub fn basic_event_count(&self) -> usize {
        self.basic_event_count
    }

    /// Index of the top-event gate.
    pub fn top(&self) -> usize {
        self.top
    }

    /// Whether every gate is coherent, so the tree has no complemented
    /// literals and [`super::mocus::minimal_cut_sets`] can analyse it.
    pub fn is_coherent(&self) -> bool {
        self.gates.iter().all(|g| g.connective.is_coherent())
    }
}

/// A fault tree, its basic-event probabilities, and the names they came from.
///
/// This is what [`FaultTreeBuilder::build`] returns. The probability vector is
/// indexed exactly as [`Arg::BasicEvent`] and [`super::probability::CutSet`]
/// members are, so it can be handed straight to
/// [`super::probability::top_event_probability`].
#[derive(Debug, Clone, PartialEq)]
pub struct FaultTreeModel {
    tree: FaultTree,
    probabilities: Vec<f64>,
    basic_event_names: Vec<String>,
    gate_names: Vec<String>,
}

impl FaultTreeModel {
    /// The validated tree.
    pub fn tree(&self) -> &FaultTree {
        &self.tree
    }

    /// Basic-event probabilities, indexed as cut-set members are.
    pub fn probabilities(&self) -> &[f64] {
        &self.probabilities
    }

    /// Basic-event names, in index order.
    pub fn basic_event_names(&self) -> &[String] {
        &self.basic_event_names
    }

    /// Gate names, in index order.
    pub fn gate_names(&self) -> &[String] {
        &self.gate_names
    }

    /// Index of a basic event by name, or `None` if the tree has no such
    /// event.
    pub fn basic_event_index(&self, name: &str) -> Option<usize> {
        self.basic_event_names.iter().position(|n| n == name)
    }
}

/// Builds a [`FaultTreeModel`] from named gates and basic events.
///
/// Declaration order does not matter: a gate may name arguments that have not
/// been declared yet, and everything is resolved and validated in
/// [`FaultTreeBuilder::build`]. That is deliberate — a fault tree is normally
/// written top-down, and requiring bottom-up declaration would make it
/// tedious to transcribe one.
///
/// A name that is never declared as either a gate or a basic event is an
/// error, not an implicit basic event. Silently inventing a leaf is how a
/// typo becomes a wrong answer.
#[derive(Debug, Clone, Default)]
pub struct FaultTreeBuilder {
    gate_names: Vec<String>,
    gate_specs: Vec<(Connective, Vec<String>)>,
    basic_event_names: Vec<String>,
    probabilities: Vec<f64>,
}

impl FaultTreeBuilder {
    /// A builder with no gates and no basic events.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a basic event and its probability of occurrence.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the name is already taken, or if
    /// `probability` is outside `[0, 1]` or not finite.
    pub fn basic_event(&mut self, name: &str, probability: f64) -> Result<usize> {
        if !(0.0..=1.0).contains(&probability) || !probability.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "probability".to_string(),
                value: probability,
                reason: format!("probability of basic event `{name}` must lie in [0, 1]"),
            });
        }
        self.reject_duplicate(name)?;
        self.basic_event_names.push(name.to_string());
        self.probabilities.push(probability);
        Ok(self.basic_event_names.len() - 1)
    }

    /// Declares a gate, its connective, and the names of its arguments.
    ///
    /// Arguments may name gates or basic events, declared before or after.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the name is already taken, or if
    /// the argument count does not suit the connective (see
    /// [`Connective`]). Unresolvable argument names are reported by
    /// [`FaultTreeBuilder::build`], since they may legitimately not exist yet.
    pub fn gate(&mut self, name: &str, connective: Connective, args: &[&str]) -> Result<usize> {
        self.reject_duplicate(name)?;
        check_arity(name, connective, args.len())?;
        self.gate_names.push(name.to_string());
        self.gate_specs
            .push((connective, args.iter().map(|a| a.to_string()).collect()));
        Ok(self.gate_names.len() - 1)
    }

    /// Resolves every name, validates the structure, and returns the model.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `top` is not a declared gate, if
    /// any argument names nothing that was declared, or if the gates form a
    /// cycle. A cycle is reported with the gate it was detected at, because a
    /// fault tree that feeds itself has no cut sets at all and would otherwise
    /// hang the generator.
    pub fn build(self, top: &str) -> Result<FaultTreeModel> {
        let gate_index: HashMap<&str, usize> = self
            .gate_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        let event_index: HashMap<&str, usize> = self
            .basic_event_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();

        let top = *gate_index
            .get(top)
            .ok_or_else(|| RafflesError::InvalidParameter {
                parameter: "top".to_string(),
                value: 0.0,
                reason: format!("`{top}` is not a declared gate"),
            })?;

        let mut gates = Vec::with_capacity(self.gate_specs.len());
        for (i, (connective, arg_names)) in self.gate_specs.iter().enumerate() {
            let mut args = Vec::with_capacity(arg_names.len());
            for arg_name in arg_names {
                let arg = if let Some(&g) = gate_index.get(arg_name.as_str()) {
                    Arg::Gate(g)
                } else if let Some(&e) = event_index.get(arg_name.as_str()) {
                    Arg::BasicEvent(e)
                } else {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "args".to_string(),
                        value: 0.0,
                        reason: format!(
                            "gate `{}` names `{arg_name}`, which is neither a declared gate \
                             nor a declared basic event",
                            self.gate_names[i]
                        ),
                    });
                };
                args.push(arg);
            }
            gates.push(Gate {
                connective: *connective,
                args,
            });
        }

        detect_cycle(&gates, &self.gate_names)?;

        Ok(FaultTreeModel {
            tree: FaultTree {
                gates,
                basic_event_count: self.basic_event_names.len(),
                top,
            },
            probabilities: self.probabilities,
            basic_event_names: self.basic_event_names,
            gate_names: self.gate_names,
        })
    }

    fn reject_duplicate(&self, name: &str) -> Result<()> {
        if self.gate_names.iter().any(|n| n == name)
            || self.basic_event_names.iter().any(|n| n == name)
        {
            return Err(RafflesError::InvalidParameter {
                parameter: "name".to_string(),
                value: 0.0,
                reason: format!(
                    "`{name}` is already declared; gate and basic-event names share one \
                     namespace, as they do in a SCRAM input model"
                ),
            });
        }
        Ok(())
    }
}

/// Checks that an argument count suits a connective.
fn check_arity(name: &str, connective: Connective, n: usize) -> Result<()> {
    let complaint = match connective {
        Connective::Not | Connective::Null if n != 1 => Some(format!(
            "`{}` takes exactly one argument, got {n}",
            connective.as_str()
        )),
        Connective::Xor if n != 2 => Some(format!("`xor` takes exactly two arguments, got {n}")),
        Connective::Atleast { min } if min == 0 || min > n => Some(format!(
            "`atleast min=\"{min}\"` needs 1 <= min <= {n} arguments, got {n}"
        )),
        _ if n == 0 => Some("a gate needs at least one argument".to_string()),
        _ => None,
    };
    match complaint {
        Some(reason) => Err(RafflesError::InvalidParameter {
            parameter: "args".to_string(),
            value: n as f64,
            reason: format!("gate `{name}`: {reason}"),
        }),
        None => Ok(()),
    }
}

/// Depth-first search for a cycle among the gates.
///
/// A cycle makes cut-set generation non-terminating, so it is caught at
/// construction rather than guarded against in the generator.
fn detect_cycle(gates: &[Gate], names: &[String]) -> Result<()> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Unvisited,
        InProgress,
        Done,
    }
    let mut mark = vec![Mark::Unvisited; gates.len()];
    // Explicit stack: a deep tree would otherwise risk overflowing the real one.
    for start in 0..gates.len() {
        if mark[start] != Mark::Unvisited {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        mark[start] = Mark::InProgress;
        while let Some(&mut (gate, ref mut next)) = stack.last_mut() {
            if *next < gates[gate].args.len() {
                let arg = gates[gate].args[*next];
                *next += 1;
                if let Arg::Gate(child) = arg {
                    match mark[child] {
                        Mark::InProgress => {
                            return Err(RafflesError::InvalidParameter {
                                parameter: "gates".to_string(),
                                value: child as f64,
                                reason: format!(
                                    "gate `{}` is part of a cycle; a fault tree must be acyclic",
                                    names[child]
                                ),
                            })
                        }
                        Mark::Unvisited => {
                            mark[child] = Mark::InProgress;
                            stack.push((child, 0));
                        }
                        Mark::Done => {}
                    }
                }
            } else {
                mark[gate] = Mark::Done;
                stack.pop();
            }
        }
    }
    Ok(())
}
