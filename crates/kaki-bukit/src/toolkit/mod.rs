// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/ (the whole directory)
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Reusable facility building blocks: the pieces every fuel-cycle archetype
//! assembles itself from.
//!
//! Upstream's `src/toolkit/` is where Cyclus keeps the things that are not the
//! kernel but that every archetype needs anyway — an inventory buffer, a
//! material inspector, the separative-work arithmetic, a commodity registry, a
//! demand curve. None of it is simulation machinery; all of it is the
//! vocabulary a facility is written in.
//!
//! # What is here
//!
//! | Module | Upstream | What it is for |
//! |---|---|---|
//! | [`mat_query`] | `mat_query.{h,cc}` | Read masses, moles and fractions out of a [`Material`](crate::material::Material) |
//! | [`enrichment`] | `enrichment.{h,cc}` | Assays, feed/tails mass balance, SWU |
//! | [`res_buf`] | `res_buf.h` | The inventory buffer every facility holds its stock in |
//! | [`total_inv_tracker`] | `total_inv_tracker.h` | A facility-wide cap across several buffers |
//! | [`commodity`] | `commodity.{h,cc}`, `commodity_producer.{h,cc}` | Named commodities, and who produces them at what capacity and cost |
//! | [`symbolic`] | `symbolic_functions.{h,cc}` | Linear / exponential / piecewise demand curves |
//! | [`position`] | `position.{h,cc}` | Latitude, longitude and great-circle distance |
//!
//! # What is NOT here, and what comes next
//!
//! **`matl_buy_policy` and `matl_sell_policy` are the intended next step.**
//! They are the two pieces that turn a [`ResBuf`](res_buf::ResBuf) into a
//! participant in the dynamic resource exchange — a buy policy posts requests
//! to fill a buffer, a sell policy posts bids to drain one — and they are
//! deliberately absent here because both need the `Agent`/`Context` framework
//! (`Agent::context()`, `Context::NewDatum`, trade callbacks) that is being
//! built separately. Once an agent can be handed a context, those two files
//! are the obvious next port and they slot straight onto [`res_buf`] and
//! [`commodity`].
//!
//! Also not ported, with reasons:
//!
//! | Upstream | Why not |
//! |---|---|
//! | `res_manip.{h,cc}` (`Squash`, `ResCast`) | `ResCast` is C++ pointer down-casting that the [`Resource`](crate::resource::Resource) enum makes unnecessary. `Squash` needs [`AtomicMasses`](crate::composition::AtomicMasses) to absorb materials, which upstream reaches through a global PyNE table; see [`res_buf`] for how the split-pop path works without it. |
//! | `building_manager`, `supply_demand_manager`, `commodity_producer_manager` | All are `Agent`-framework managers. |
//! | `symbolic_function_factories` | XML-input parsing for the function types in [`symbolic`]. There is no XML layer in this kernel; build the enums directly. |
//! | `timeseries`, `infile_converters`, `*.cycpp.h` | Output recording and the C++ preprocessor's generated state, neither of which exists here. |
//! | `agent_managed`, `commodity_recipe_context` | `Agent` framework. |
//!
//! # Translation rules, applied here
//!
//! The workspace forbids trait objects, `Box` and lifetime parameters on
//! structs. Each of those appears in this directory upstream, and the
//! substitution is the same one the crate root documents:
//!
//! | Upstream C++ | Here |
//! |---|---|
//! | `boost::shared_ptr<SymFunction>` hierarchy | [`SymFunction`](symbolic::SymFunction), an enum matched at each call |
//! | `ResBuf<T>` template over `Material`/`Product` | [`ResBuf`](res_buf::ResBuf) holding the [`Resource`](crate::resource::Resource) enum |
//! | `std::list<Resource::Ptr>` with a `std::set` duplicate guard | a `VecDeque<Resource>`; ownership makes a duplicate push unrepresentable |
//! | `std::vector<ResBuf<Material>*>` | a borrowed slice passed per call |
//! | thrown `ValueError` | [`Result<T>`](crate::error::Result) |

pub mod commodity;
pub mod enrichment;
pub mod mat_query;
pub mod position;
pub mod res_buf;
pub mod symbolic;
pub mod total_inv_tracker;
