//! Verification & validation gates and the committed oracle tables this crate is
//! measured against.
//!
//! The gate helpers themselves live one crate down, in
//! [`njoy_outram_park_fork::vv`], because both crates run oracle comparisons and
//! this one depends on that one — so there is a single implementation rather
//! than two that drift. They are re-exported here so callers need only one path.
//!
//! [`njoy_golden`] holds the NJOY2016 oracle **values** for the comparisons that
//! are about *this* crate's fidelity: the S(alpha,beta) laws and the U-238 point
//! cross sections. Those belong here rather than in the data crate, because what
//! they measure is `outram-mc-libs` reproducing NJOY, not NJOY reproducing
//! itself.

pub mod njoy_golden;

pub use njoy_outram_park_fork::vv::{
    assert_absolute, assert_monotone, assert_relative, assert_reproduces_keff,
    assert_reproduces_recorded, assert_table_relative, WorstDeviation,
};
