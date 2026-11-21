//! `HEN_HOUSE/src/eii.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! Electron-impact ionisation cross sections from a generalised-oscillator-strength approach, and the sampling that uses them (369 lines). Description from the file's header and size; not yet read line by line.
//!
//! **Status: ported, in `egsnrc_mortran.rs` (corrected 2026-11-21).** `eii_init` is [`super::egsnrc_mortran::eii_init`], `eii_sample` is [`super::egsnrc_mortran::eii_sample`], and the EII selection that opens `MOLLER` is [`super::egsnrc_mortran::moller_eii_select`], with `EiiReadings` for the four oddities in `eii.mortran` (see `docs/egsnrc_port_remaining.md`). They live beside the routines that call them rather than in this file. **CORRECTED 2026-11-21 (gh:#1689):** `HEN_HOUSE/src/eii.mortran` is **not part of the build**; the built program compiles `eii_init`/`eii_sample` that are written inside `egsnrc.mortran` (`:7433` and `:7759`), and they differ from this file (new Moller-subtraction formulas, `ebr1` rewritten, `esig_e`, the other `eii_sample` sampling, `pese1 = peie - wx`). The live versions are [`super::egsnrc_mortran::eii_init_egsnrc`] and [`super::egsnrc_mortran::eii_sample_egsnrc`] (now what `moller_eii_select` calls), differentially tested against the built program (`docs/egsnrc_phase2_differential.md`); the ports of this file's text remain as `eii_init`/`eii_sample`. A first port, not differentially tested (gh:#1680).
