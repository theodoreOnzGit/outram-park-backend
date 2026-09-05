pub mod second_order_transfer_fn;
pub mod first_order_transfer_fn;
pub mod decaying_sinusoid;
pub mod first_order_transfer_fn_with_zeroes;
pub mod step_fn;

/// Verification and regression tests for the exact O(1) recurrences that
/// replaced the growing response vectors in this module tree (bead `op-fm5`).
#[cfg(test)]
mod recurrence_tests;
