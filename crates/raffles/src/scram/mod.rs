// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c
//   Commit date:      2019-07-03
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//
//   This program is free software; you can redistribute it and/or modify
//   it under the terms of the GNU General Public License as published by
//   the Free Software Foundation; either version 3 of the License, or
//   (at your option) any later version.
//
//   This program is distributed in the hope that it will be useful,
//   but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//   GNU General Public License for more details.
//
// SCRAM is GPL-3.0-or-later and RAFFLES is GPL-3.0-only, so this is a
// same-licence port, NOT the one-way Apache-2.0 -> GPLv3 relicensing that
// governs the RAVEN-derived parts of this crate. Code may flow SCRAM ->
// RAFFLES freely under GPLv3. Contributing anything back upstream is still a
// decision for the crate owner and the copyright holders, not an assistant.
// ---------------------------------------------------------------------------

//! # SCRAM port — fault-tree quantification
//!
//! A translation of the probabilistic-risk-analysis core of
//! [SCRAM](https://github.com/rakhimov/scram): taking the **minimal cut sets**
//! of a fault tree and turning them into a top-event probability and a ranking
//! of which basic events matter.
//!
//! ## What a cut set is, for a reader who has not met one
//!
//! A **fault tree** is a Boolean expression for how a system fails, written
//! over *basic events* — a pump not starting, a valve stuck shut. A **cut set**
//! is a set of basic events whose simultaneous occurrence is sufficient to
//! cause the top event. It is **minimal** when removing any member stops it
//! being sufficient.
//!
//! The minimal cut sets are the complete qualitative answer: the system fails
//! exactly when at least one of them occurs. The quantitative questions — how
//! likely, and which events drive it — follow from them.
//!
//! ## The route through this module
//!
//! 1. [`fault_tree::FaultTreeBuilder`] — describe the tree in names.
//! 2. [`mocus::minimal_cut_sets`] — generate the cut sets, or
//!    [`zbdd::minimal_cut_sets`] when the model is big enough that top-down
//!    expansion gives up.
//! 3. [`top_event_probability`] — quantify.
//! 4. [`importance_factors`] — rank the basic events.
//!
//! **If all you want is the probability, skip to [`bdd::Bdd`].** It evaluates
//! the tree's Boolean function directly, so it needs no cut sets, has no
//! `2^n` ceiling, and on a non-coherent tree gives the *true* value where cut
//! sets can only bound it. Cut sets remain the answer to "how does it fail",
//! which no single probability can give.
//!
//! Steps 3 and 4 are **ports** of SCRAM and carry its attribution headers.
//! Step 2 is **not**: SCRAM generates cut sets with a ZBDD over a
//! heavily-preprocessed Boolean graph, which is the larger part of the
//! upstream and is not ported. [`mocus`] is the classical top-down expansion
//! from the published literature instead, verified *against* SCRAM's reported
//! products rather than translated from its code — see that module.
//!
//! A caller who already has cut sets from elsewhere can skip straight to
//! step 3; [`CutSet`] does not care where they came from.
//!
//! **Non-coherent trees are handled, and there the answer you ask for
//! matters.** Where a `not`, `nand`, `nor` or `xor` appears, a component
//! *working* can contribute to the top event. Minimal cut sets then discard
//! that information and become **conservative** — quantifying them bounds the
//! probability from above. Two things recover the exact answer:
//! [`bdd::Bdd::probability`] for the probability itself, and
//! [`zbdd::prime_implicants`] for the combinations, which keep the
//! complemented literals. Measured on the fixture's small non-coherent model:
//! cut sets sum to `0.80`, prime implicants to `0.54`, and the truth is
//! `0.5032`.
//!
//! **Reading SCRAM's own input models.** [`mef`] takes a Model Exchange
//! Format document to a [`fault_tree::FaultTreeModel`], evaluating the
//! basic-event expressions through [`expression`] on the way. It handles
//! `<define-component>`'s private namespaces, `<xi:include>`, and all eleven
//! MEF connectives, and it **refuses** rather than skips anything it does not
//! read.
//!
//! **Common-cause failure groups are applied by default.** A model that
//! declares one has said its components are coupled, and [`ccf`] rewrites the
//! tree accordingly — each member becomes a proxy gate over the shared-failure
//! events it belongs to. Upstream does this only under `scram --ccf`;
//! [`mef::MefModel::without_ccf`] is the explicit ablation, and both paths are
//! verified against the corresponding SCRAM run. On
//! `TwoTrain/common_cause` the difference is `0.0622587` against `0.0361` —
//! ignoring a declared group is not a small approximation.
//!
//! ~~**What is still absent:** everything SCRAM does around this core — XML
//! input models, … and the expression library~~ **CORRECTED 2026-09-22** —
//! both landed, and on 2026-09-22 so did the seven random deviates and the
//! four common-cause-failure models ([`ccf`]) and the substitutions
//! ([`substitution`]). What is still absent: event trees, alignments,
//! **sampling** and the uncertainty analysis over it, the
//! reporter, and — in the analysis itself — the preprocessor, whose absence is
//! a cost in diagram size rather than in answers. A deviate evaluates to its
//! mean, which is what an ordinary SCRAM run computes; nothing here draws
//! from a distribution.
//!
//! ## Where this sits relative to the rest of the crate
//!
//! [`crate::imprecise::SystemStructure`] also computes system reliability, and
//! the two are **not** duplicates:
//!
//! | | `imprecise::SystemStructure` | this module |
//! |---|---|---|
//! | input | component reliabilities | minimal cut sets + event probabilities |
//! | structure | series, parallel, k-out-of-n | arbitrary coherent fault tree |
//! | numbers | **interval-valued** | point-valued |
//! | question | reliability under dependence assumptions | top-event probability and event importance |
//!
//! Reach for `imprecise` when the structure is simple and the inputs are
//! bounds; reach for this when the structure is a real fault tree.
//!
//! ## Verification
//!
//! Every function here is checked against **upstream SCRAM compiled and run**,
//! not against a reading of its source. The oracle harness, the exact upstream
//! commit, the one build patch that was needed, and the measured agreement are
//! recorded in `crates/raffles/docs/scram-port-verification.md`.

pub mod bdd;
pub mod ccf;
pub mod expression;
pub mod fault_tree;
pub mod importance;
pub mod mef;
pub mod mocus;
pub mod probability;
pub mod substitution;
pub mod zbdd;

pub use bdd::Bdd;
pub use expression::Expression;
pub use mef::MefModel;
pub use fault_tree::{Arg, Connective, FaultTree, FaultTreeBuilder, FaultTreeModel, Gate};
pub use importance::{importance_factors, importance_factors_from_bdd, ImportanceFactors};
pub use mocus::minimal_cut_sets;
pub use zbdd::{count_minimal_cut_sets, minimal_cut_sets_from_graph, prime_implicants, PrimeImplicant};
pub use probability::{cut_set_probability, top_event_probability, Approximation, CutSet};
