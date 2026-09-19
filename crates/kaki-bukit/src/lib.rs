// OUTRAM PARK — independent Rust translation of CYCLUS and CYCAMORE.
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
// GPL-3.0-only. Contains work derived from CYCLUS and CYCAMORE
// (both BSD-3-Clause); see NOTICE for provenance and per-file attribution.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Validation guards in this crate are written `if !(x > 0.0)`, never
// `if x <= 0.0`. The two differ on NaN: `!(NaN > 0.0)` is `true`, so the
// guard rejects NaN, while `NaN <= 0.0` is `false`, so the rewrite clippy
// suggests would let a NaN quantity, preference or capacity through into the
// exchange solver. A NaN there does not fail loudly -- it propagates into a
// capacity comparison, makes every match test false, and the trade silently
// does not happen. Rejecting it at construction is the whole point, so the
// lint is off crate-wide rather than argued with five times.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

//! An independent, `no_std` Rust translation of **CYCLUS**, the agent-based
//! nuclear fuel-cycle simulator, and **CYCAMORE**, its library of fuel-cycle
//! facility agents.
//!
//! <!-- vv-unverified-banner -->
//! > ⚠️ **Unverified until validated.** All code in this workspace is
//! > **unverified and untrusted** unless a specific verification & validation
//! > (V&V) case demonstrates otherwise. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.
//!
//! # What Cyclus is, in one paragraph
//!
//! A fuel-cycle simulation is a set of facilities — mines, enrichment plants,
//! reactors, repositories — that pass material to each other over discrete
//! time steps. Cyclus's distinctive idea is that it does *not* hard-wire who
//! sends what to whom. Each time step it runs a **dynamic resource exchange**:
//! consumers post [requests](exchange::request), producers answer with
//! [bids](exchange::bid), each side declares its capacity constraints, and a
//! solver picks a set of trades. The facilities are agents; the market between
//! them is recomputed every step. That exchange is the heart of the code and
//! the heart of this translation.
//!
//! # Where to start reading
//!
//! Follow the material, not the module list:
//!
//! 1. A [`Nuc`](nuclide::Nuc) is a nuclide id; a [`CompMap`](comp_math::CompMap)
//!    maps nuclides to dimensionless quantities.
//! 2. A [`Composition`](composition::Composition) fixes whether those
//!    quantities are atom or mass fractions, and can convert between them.
//! 3. A [`Material`](material::Material) is a quantity in kilograms plus a
//!    composition. [`Product`](product::Product) is the same idea for things
//!    that are not nuclides.
//! 4. Facilities hold resources in a [`ResBuf`](toolkit::res_buf::ResBuf).
//! 5. Each time step, [`exchange`] matches requests to bids.
//!
//! # `no_std`
//!
//! This crate is `#![no_std]` and depends on `alloc`. Fuel-cycle bookkeeping is
//! irreducibly dynamic — a composition is a map whose size is not known until
//! run time — so `alloc` is required; `std` is not, and is never used. All
//! numerics come from [`petir`], which is itself `no_std`, so a fuel-cycle
//! model can be embedded in a controller, compiled to wasm for a browser, or
//! run on Android without a second implementation of anything.
//!
//! Because PETIR's transcendentals are one fixed pure-Rust implementation
//! rather than whatever libm the platform ships, two runs of the same input on
//! different machines produce **bit-identical** results. For a code whose
//! output is a mass balance someone will cite, that is worth the constraint.
//!
//! # Scope: what is NOT ported, and why
//!
//! This crate is the **simulation kernel only**. The following upstream
//! subsystems are deliberately absent, and their absence is a design decision
//! rather than an unfinished task:
//!
//! | Upstream | Why it is not here |
//! |---|---|
//! | `hdf5_back`, `sqlite_back`, `recorder` | Output persistence. Both are C library bindings; neither can be `no_std`. |
//! | `xml_file_loader`, `xml_parser`, `infile_tree` | Input decks, via libxml2. Same reason. |
//! | `prog_solver`, `prog_translator`, `OsiCbcSolverInterface` | The mixed-integer LP exchange solver, via Coin-OR/Cbc. A C++ dependency an order of magnitude larger than this crate. The [greedy solver](exchange::greedy) *is* ported, and is upstream's default. |
//! | `dynamic_module`, `discovery` | Loading agent archetypes from shared libraries at run time. Rust resolves the equivalent at compile time, through the [`AgentKind`](agents::AgentKind) enum. |
//! | `pyhooks`, `pymodule`, `pyinfile`, the Cython layer | Python bindings. |
//! | `Decayer` and its bundled chain data | Deprecated upstream in favour of PyNE's decay. More importantly, the workspace rule is that nuclear data belongs in `njoy-outram-park-fork`, so [`decay`] takes a caller-supplied [`DecayChain`](decay::DecayChain) and ships no data of its own. |
//!
//! A downstream crate that wants persistence or XML input adds it on top; the
//! kernel does not need to know.
//!
//! # Translation rules this crate follows
//!
//! The workspace forbids trait objects, `Box`, and lifetime parameters. Cyclus
//! is built from all three — `Agent*` polymorphism, `boost::shared_ptr`
//! resource graphs — so the translation is not mechanical, and the substitutions
//! are uniform enough to state once:
//!
//! | Upstream C++ | Here |
//! |---|---|
//! | `virtual` dispatch over `Agent*` | [`AgentKind`](agents::AgentKind), an enum matched at each call |
//! | `boost::shared_ptr<ExchangeNode>` | [`NodeId`](exchange::graph::NodeId), an index into an arena |
//! | `Resource::Ptr` | [`Resource`](resource::Resource), an owned enum |
//! | `std::map<Nuc, double>` | [`CompMap`](comp_math::CompMap), a `BTreeMap` — ordered, so results reproduce |
//! | thrown exceptions | [`Result<T>`](error::Result) |
//!
//! Every ported file carries a `PROVENANCE` header block naming the upstream
//! file and commit, so any routine here can be opened next to its source and
//! read line for line.

extern crate alloc;

pub mod agent;
pub mod agents;
pub mod arithmetic;
pub mod comp_math;
pub mod composition;
pub mod context;
pub mod decay;
pub mod error;
pub mod exchange;
pub mod limits;
pub mod material;
pub mod nuclide;
pub mod product;
pub mod resource;
pub mod sim;
pub mod toolkit;

pub use error::{CyclusError, Result};
pub use nuclide::Nuc;
