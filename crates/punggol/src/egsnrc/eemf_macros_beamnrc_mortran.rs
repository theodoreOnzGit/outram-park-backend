//! `HEN_HOUSE/src/EEMF_macros_beamnrc.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! The BEAMnrc variant of `EEMF_macros.mortran` (2546 lines).
//!
//! **Not ported (2026-11-21), after an attempt; the blocker is size, not data.**
//! ~~Not ported because it needs BEAMnrc component modules~~ **CORRECTED
//! 2026-11-21:** BEAMnrc is public; its sources were fetched (`git
//! sparse-checkout add HEN_HOUSE/omega HEN_HOUSE/utils`) and a one-slab-CM
//! accelerator was **built and run** with this file as its field macros (recipe
//! and results: `docs/egsnrc_phase2_differential.md`, "EEMF_macros_beamnrc").
//! What the comparison showed about the variant:
//! - the **only host dependency** of the file is `$CALL-HOWNEAR_EM(n, tperp)`
//!   (BEAMnrc: the component module's `HOWNEAR` through `IR_to_CM`/`$GOTO_CM_LIST`;
//!   a user code can define it as a plain `HOWNEAR` call) and the BEAMnrc common
//!   variable `EIN`, read only in an error message of the electric-field root
//!   finder;
//! - **magnetic field in a medium: bit-identical to `EEMF_macros.mortran`**
//!   (tutor1 host, B = (0, 0.5, 0.8) T: 200 of 200 histories, worst difference 0);
//!   `$EMFIELD_PII`, `$TUSTEP_RIST_B`, `$NORM_VEC_B`, `$INTERACT_EM` and the
//!   magnetic single-scatter branch are the same text up to dead code;
//! - **in vacuum it is a different algorithm** (`$EMFieldInVacuum`: a
//!   `HOWNEAR` call before the first branch, `x0_em`-based linear steps, no
//!   `HOWFAR` probe, `$CALL-HOWNEAR_EM` at five numbered sites): in the layered
//!   vacuum-gap host only 41 to 69 of 200 histories keep the charge sequence;
//! - the electric field is a different algorithm too (`$SS_EF_Transport`
//!   expanded with `$EF_timeRootFinder`, Simpson over energy, `$ERMJ`, `$ERM_kg`
//!   in SI units, `$EF_simpsonOverEnergy`, `$EF_velocityFunction`,
//!   `$EF_distanceFunction`), ~600 lines that exist only in this variant.
//!
//! What a port still needs: the vacuum macro of the variant (B branch, ~180
//! lines), the `IF(medium ~= 0)` block of `$EMFIELD_INITIATE_SET_TUSTEP`, the
//! numbered `$CALL-HOWNEAR_EM` sites in a `Geometry` seam, and the electric
//! transport above; then a host comparison in the layered geometry. It was not
//! done inside the time box.
//!
//! **Status: placeholder, nothing ported (gh:#976).** When code lands here it
//! carries the attribution header block (upstream project, file, commit,
//! copyright, licence) and a doc-comment `file:line` for every routine.
