// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Module root. The filters are ported from FilterPy (MIT) and each ported
// file carries its own header; this file and numpy_ops.rs / consistency.rs
// are not ports. Added 2026-11-04 for GitHub issue #1191 (epic #1177) at the
// workspace maintainer's direction.

//! **State estimation: FilterPy's Kalman filters** (gh:#1191).
//!
//! Ported from FilterPy (R. Labbe, <https://github.com/rlabbe/filterpy>,
//! commit `5df4e095fac0`, 2.0.0, MIT; `LICENSE-MIT-FILTERPY`, crate `NOTICE`):
//!
//! | Item | Upstream |
//! |---|---|
//! | [`KalmanFilter`], [`rts_smoother`] | `kalman/kalman_filter.py`: predict, Joseph-form update, batch filter, RTS |
//! | [`ExtendedKalmanFilter`] | `kalman/EKF.py` |
//! | [`MerweScaledSigmaPoints`] | `kalman/sigma_points.py` |
//! | [`UnscentedKalmanFilter`], [`unscented_transform`] | `kalman/UKF.py`, `kalman/unscented_transform.py` |
//! | [`EnsembleKalmanFilter`], [`outer_product_sum`] | `kalman/ensemble_kalman_filter.py`, `common/helpers.py` |
//! | [`q_discrete_white_noise`], [`q_continuous_white_noise`], [`order_by_derivative`] | `common/discretization.py` |
//!
//! Written here, not ported: [`Matrix`] and the kernels of
//! [`numpy_ops`], which round as numpy/OpenBLAS do (measured, so the
//! filters are bit-identical with FilterPy inside the measured envelope);
//! [`gaussian_draws`] (the EnKF's own draws on the workspace LCG);
//! [`nees`] and [`nis`].
//!
//! The typed route for a caller's model is
//! [`crate::fidelity::StateSpaceModel`] with [`crate::fidelity::Assimilator`].
//!
//! **Synthetic data only** (`RESPONSIBLE_USE.md`): these filters are for
//! twin experiments, teaching and V&V, never for estimating the state of a
//! real plant.
//!
//! V&V: `crates/raffles/verification_and_validation/filterpy_gh1191.md`,
//! tests `tests/filterpy_gh1191.rs`.

mod consistency;
mod discretization;
mod ekf;
mod enkf;
mod kalman;
pub mod numpy_ops;
mod sigma_points;
mod ukf;

pub use consistency::{nees, nis};
pub use discretization::{order_by_derivative, q_continuous_white_noise, q_discrete_white_noise};
pub use ekf::ExtendedKalmanFilter;
pub use enkf::{gaussian_draws, outer_product_sum, EnsembleKalmanFilter};
pub use kalman::{rts_smoother, BatchOutput, KalmanFilter, RtsOutput};
pub use numpy_ops::Matrix;
pub use sigma_points::MerweScaledSigmaPoints;
pub use ukf::{unscented_transform, UnscentedKalmanFilter};
