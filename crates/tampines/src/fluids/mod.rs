//! Fluid selection unified across TAMPINES's backend property libraries.
//!
//! This module belongs to the *fluid-selection* concern only -- picking which
//! backend evaluates a fluid's thermophysical properties. It does not itself
//! hold flow/loop state; see [`crate::single_phase`] and
//! [`crate::compressible`] for the component-level wrappers that use a
//! [`TampinesFluid`] to construct a loop segment.

use outram_park_fork_coolprop::Fluid as CoolPropFluid;

/// Which backend thermophysical-property source a TAMPINES fluid is
/// evaluated with.
///
/// This is a *property-backend* selector, not an independent substance list:
/// [`TampinesFluid::CoolProp`] carries an
/// [`outram_park_fork_coolprop::Fluid`] (137 fluids, including water, air,
/// nitrogen, helium, ...); [`TampinesFluid::Steam`] instead routes to the
/// dedicated IAPWS-IF97 `tampines-steam-tables` backend, this workspace's own
/// water/steam implementation, rather than CoolProp's own water equation of
/// state.
///
/// **V&V status of that backend**, re-read from its test source on
/// 2026-08-11 -- state it accurately rather than calling it "validated"
/// wholesale:
///
/// - **IF97 property regions** -- verified against the Kretzschmar & Wagner
///   (2019) IAPWS-IF97 reference tables in the per-region test modules.
/// - **Moody (1975) Fig. 1 choked flow** -- verified: 13 active isobar tests
///   (`moody_critical_mass_flux_homogeneous_eqm.rs`, p0/p_ref = 0.25-30.0)
///   assert `|log10 G_test - log10 G_ref| <= 0.06`, loosened to 0.08 for
///   deeply-subcooled Region-1 points. The validator is region-filtered:
///   points that are neither in-dome (Region 4) nor deeply subcooled are
///   skipped as a documented HEM limitation, not asserted.
/// - **Zaloudek choked flow** -- verified: 89 tests across five files
///   (21 in-dome / 21 generic-dispatcher / 21 backward-throat / 22
///   subcooled, one of which is an `#[ignore]`d diagnostic sweep / 4
///   superheated), at critical-pressure tolerance 0.005-0.05 by curve and
///   mass-flux log10 tolerance 0.05. Caveat: the reference curves are
///   graph-read HEM curves digitised from Saha (1978) NUREG/CR-0417, **not**
///   raw experimental data.
/// - **Marviken is NOT validated.** The digitised NUREG/CR-2671 test-23/24
///   data sits in `marviken_tests.rs`, but the single test is
///   `#[ignore = "skip first, Marviken is more complex"]`, its only
///   assertion is commented out, and the body ends in `todo!()`. Marviken is
///   **not** a basis for choosing this backend. Status as of 2026-08-11:
///   gating is being implemented in `tampines-steam-tables` (the follow-up
///   to op-bcg/op-4ily); re-check that crate's test suite for the current
///   result rather than treating this line as permanent.
///
/// Separately, the Edwards-O'Brien blowdown benchmark **is** genuinely gated
/// -- 2 active, passing tests in `tampines-steam-tables`'s
/// `tests/edwards_blowdown.rs` -- but that exercises the transient pipe
/// solver in [`crate::multiphase_1d`], not the property backend selected
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TampinesFluid {
    /// Evaluate via the CoolProp-derived backend (`outram-park-fork-coolprop`).
    CoolProp(CoolPropFluid),
    /// Evaluate water/steam via the IAPWS-IF97 `tampines-steam-tables` backend.
    Steam,
}
