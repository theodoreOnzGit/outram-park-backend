//! Pebble-bed reactor specialization — the doubly-heterogeneous transport slice.
//!
//! This module is where `outram-mc-libs` deliberately specializes. The fork's target
//! application is the **pebble-bed reactor**, whose geometry is *doubly
//! heterogeneous*: thousands of sub-millimetre TRISO fuel particles are packed in a
//! graphite matrix to form a pebble, and tens of thousands of pebbles are packed
//! into the core. A pebble holds O(10⁴) TRISO kernels; a core holds O(10⁵) pebbles —
//! so a naive surface-tracking transport sweep would spend all its time computing
//! distances to an astronomical number of spherical shells.
//!
//! Two families of technique make this tractable, and both live here:
//!
//! - **[`delta_tracking`]** — Woodcock (delta) tracking. Instead of finding the
//!   next material boundary, sample a flight on a *majorant* cross section that
//!   bounds every material, then accept the landing as a real collision with
//!   probability Σ_t(local)/Σ_maj (else it is a virtual collision and the flight
//!   continues). The neutron never needs to know how many TRISO surfaces it flew
//!   past — the dominant win for doubly-heterogeneous media.
//! - **[`stochastic_media`]** — generating and sampling the random packed geometry
//!   itself. Random Sequential Addition (RSA) is implemented
//!   ([`stochastic_media::pack_spheres`]); Chord Length Sampling and the
//!   RSA–DEM/ODR–DEM high-density hybrids are future work. See the [`references`]
//!   bibliography.
//! - **[`keff_delta`]** — the assembly: a fission-source k-eigenvalue power
//!   iteration over a reflective cube of packed kernels, with every history
//!   streamed by delta tracking ([`keff_delta::run_keff_delta`]). This is the
//!   doubly-heterogeneous k-eff the other two modules exist to make tractable.
//!
//! # Why a dedicated module
//!
//! The rest of the crate is a faithful, application-neutral port of OpenMC. The
//! physics and geometry primitives it provides (surfaces, cells, universes,
//! lattices, the transport loop) are reused unchanged. What is *specialized* — the
//! majorant construction, the delta-tracking flight, and the stochastic-media
//! geometry generators optimized for high TRISO packing fractions — is quarantined
//! here so the specialization intent is explicit and the general port stays general.
//!
//! # References
//!
//! The stochastic-media geometry work draws on Zhe Chuan Tan et al.'s dispersion-fuel
//! papers in the RMC code; the machine-readable citations are in [`references`].

pub mod delta_tracking;
pub mod keff_delta;
pub mod references;
pub mod stochastic_media;
