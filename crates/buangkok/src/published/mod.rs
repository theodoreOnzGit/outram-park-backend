//! Published dose tables, stored as cited reference data. **Nothing here
//! computes a dose.**
//!
//! Moved from `changi::activity` on 2026-09-28 (maintainer: "move table 7 and
//! 9 to buangkok"), because dose belongs in the dose crate:
//!
//! - [`normal_operation_dose_by_distance`](crate::published::normal_operation_dose_by_distance) — Liu and Cao (2002), Table 7:
//!   HTR-10 normal-operation individual effective dose vs distance (mSv/a).
//! - [`accident_dose_by_distance`](crate::published::accident_dose_by_distance) — Liu and Cao (2002), Table 9: HTR-10
//!   design-basis-accident thyroid and whole-body doses vs distance (mSv).
//!
//! Both are published model results (AIRDOS-EPA and STOERNEU respectively),
//! not measurements. Provenance: `crates/buangkok/docs/References.md`.
//! `RESPONSIBLE_USE.md` applies in full.

pub mod accident_dose_by_distance;
pub mod normal_operation_dose_by_distance;
