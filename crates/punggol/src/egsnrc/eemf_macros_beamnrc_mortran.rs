//! `HEN_HOUSE/src/EEMF_macros_beamnrc.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! The BEAMnrc variant of `EEMF_macros.mortran` (2546 lines).
//!
//! **Status: first port, compared with a compiled EGSnrc (gh:#1697)**; the code
//! lives in `eemf_macros_mortran.rs` as `EemfVariant::BeamNrc` (the two files
//! share most text, so the standard code is reused and the variant branches on
//! `EemfField::variant`): `emfield_in_vacuum_beamnrc`, `em_field_ss_e_beamnrc`,
//! `ss_ef_transport` with `ef_velocity_function`, `ef_distance_function`,
//! `ef_time_root_finder`, `asinh_glibc`, and the `IF(medium~=0)` block of
//! INITIATE (`InitiateInput::in_medium`). This file holds no code on purpose.
//!
//! ~~**Not ported (2026-11-21), after an attempt; the blocker is size, not
//! data.**~~ **CORRECTED 2026-11-21 (later):** ported. Measured against the
//! same-host Fortran (tutor1 and the layered vacuum-gap code compiled with this
//! file, `$CALL-HOWNEAR_EM` a plain `HOWNEAR`, 200 histories): magnetic field
//! in a medium bit-level (<= 5e-15), in vacuum <= 1.5e-11; electric and
//! combined fields: same charge sequence in 200 of 200, energy 5e-13 to 1e-10,
//! |dw| up to 1.1e-8, which compiled-Fortran one-ulp input probes (6e-9 to
//! 2e-7) show to be round-off amplification through `asinh`/`sqrt`
//! cancellation. Tables, probe and recipe: `docs/egsnrc_phase2_differential.md`,
//! "EEMF_macros_beamnrc".
//!
//! ~~**Status: placeholder, nothing ported (gh:#976).**~~ **CORRECTED
//! 2026-11-21 (later):** first port, above.
