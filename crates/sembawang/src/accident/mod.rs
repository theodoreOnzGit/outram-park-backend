// SPDX-License-Identifier: GPL-3.0

//! Accident-phase release, orchestrated over `boon-lay`'s TRISO-ATOPS fork.
//!
//! This module **computes no release physics**. Every diffusion coefficient,
//! release fraction and breakthrough model lives in
//! `boon_lay::triso_atops_fork` and is called from here — the workspace rule is
//! reuse before porting and porting before writing, and the release layer is
//! already ported, `uom`-typed and code-to-code verified against upstream.
//!
//! What this module owns is the parts that sit *around* that: pairing the
//! venting selection with the right temperatures ([`venting`]), and assembling
//! the result into a [`changi::activity::source::SourceTerm`].
//!
//! The temperature transient itself is **prescribed by the caller** — see the
//! crate docs for the full list of what is not computed here.

pub mod release;
pub mod venting;

pub use release::{accident_release, AccidentRelease, PlantParameters};
pub use venting::VentingWindow;
