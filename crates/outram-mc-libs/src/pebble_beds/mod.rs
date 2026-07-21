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
//!   ([`stochastic_media::pack_spheres`]); the RSA–DEM/ODR–DEM high-density hybrids
//!   are future work. See the [`references`] bibliography.
//! - **[`keff_delta`]** — the assembly: a fission-source k-eigenvalue power
//!   iteration over a reflective cube of packed kernels, with every history
//!   streamed by delta tracking ([`keff_delta::run_keff_delta`]). This is the
//!   doubly-heterogeneous k-eff the other two modules exist to make tractable.
//!
//! # Stochastic-media research track (scaffold)
//!
//! Alongside the explicit-geometry path above sits a second family of methods that
//! trade stored geometry for sampled geometry, scaffolded per the *OUTRAM-MC Design
//! Scaffold v0.1* and tracked under beads epic `op-eby`:
//!
//! - **[`medium`]** — the unifying [`medium::StochasticMedium`] enum: one
//!   "which material is here?" query across the explicit and sampled models.
//! - **[`cls`]** — Chord Length Sampling: no stored geometry, chords re-sampled from
//!   closed-form statistics. Memoryless, cheap, approximate.
//! - **[`scls`]** — Semi-Implicit CLS: CLS plus a bounded window of remembered
//!   inclusions, governed by the Dynamic Inclusion Sphere
//!   ([`scls::InclusionSphere`], radius `λ_TMFP + R_largest`). The primary research
//!   target.
//! - **[`spatial_index`]** — the acceleration seam for SCLS history lookup, the
//!   anticipated bottleneck.
//!
//! **Status: scaffold.** The chord statistics, the retention machinery and the
//! brute-force index are implemented and unit-tested; the CLS/SCLS *transport
//! drivers* are not, and the not-yet-built paths return typed
//! `NotImplemented` errors rather than fabricated answers. No accuracy claim is made
//! for CLS or SCLS until the benchmark suite (bead `op-eby.7`) measures one.
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

pub mod cls;
pub mod delta_tracking;
pub mod keff_delta;
pub mod medium;
pub mod references;
pub mod scls;
pub mod spatial_index;
pub mod stochastic_media;
