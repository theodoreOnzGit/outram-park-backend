//! Verification tests for the experimental Chebyshev backward correlations.
//!
//! These are **round-trip characterisation tests against this crate's own
//! forward equations**, which is the only reference available: IAPWS publishes
//! no Region 5 backward equations to compare against, and the near-critical
//! Region 4 `(h,s)` correlations are likewise in-house. A passing test here
//! therefore says "the correlation reproduces this crate's forward equations to
//! the stated accuracy", not "the correlation agrees with IAPWS".
//!
//! Each test states its methodology and its measured results in its own doc
//! comment, per the workspace V&V documentation rule.

mod p_rho_h;
mod region_4_near_critical_hs;
mod region_5_t_ph_ps;

mod vv_report;
