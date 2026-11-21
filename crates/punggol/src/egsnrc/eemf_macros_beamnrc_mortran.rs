//! `HEN_HOUSE/src/EEMF_macros_beamnrc.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! The BEAMnrc variant of `EEMF_macros.mortran` (2546 lines).
//!
//! **Not ported (2026-11-21), and not "the same algorithm plus plumbing":**
//! `diff EEMF_macros.mortran EEMF_macros_beamnrc.mortran` is 1359 lines. Besides
//! the BEAMnrc plumbing (`$CALL-HOWNEAR_EM`, which walks the component-module
//! lists `IR_to_CM`/`$GOTO_CM_LIST` that this crate does not have; `howfarCheck`,
//! `hownearCheck`), it changes the algorithm: `$EM_FIELD_SS` takes its linear
//! branch on `(vstep <= lin_dist) & (tperp <= lin_buffer)` without the
//! `irl ~= ir(np)` clause of the standard file, its `$ssFieldAlg` tests differ
//! (`= 1` where the standard file has `> 0`), `$EMFIELD_INITIATE_SET_TUSTEP`
//! gains an `IF(medium~=0)` and a `howfarCheck`, and the electric-field
//! transport is a different code. A port needs BEAMnrc's geometry first.
//!
//! **Status: placeholder, nothing ported (gh:#976).** When code lands here it
//! carries the attribution header block (upstream project, file, commit,
//! copyright, licence) and a doc-comment `file:line` for every routine.
