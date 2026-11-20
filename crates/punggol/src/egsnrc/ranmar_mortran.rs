//! `HEN_HOUSE/src/ranmar.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! The RANMAR random number generator routines (149 lines). See `ranmar_macros`: generators go to `petir`. Description from the file's header and size; not yet read line by line.
//!
//! **Status: not re-ported, deliberately (gh:#1680).** RANMAR is already
//! ported in PETIR as [`petir::rng::ranmar`], reachable under the EGSnrc name as
//! [`petir::egsnrc`]. A second implementation would drift from the first (the
//! workspace's search-before-building rule), so this module holds no code.
//! Differential testing of PETIR's generator against a built EGSnrc belongs to
//! phase 2.
