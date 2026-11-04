// ---------------------------------------------------------------------------
// Ported from SMT, the Surrogate Modeling Toolbox (module layout only; each
// submodule carries its own header naming the files it ports).
//
//   Upstream repo:    https://github.com/SMTorg/smt
//   Upstream commit:  151a3ff3c13733d168b0ca8844ff0a4ff072252a  (master)
//   Accessed:         2026-11-04
//   Copyright (c) 2017, SMT developers. BSD 3-Clause License
//   (LICENSE-BSD-SMT; NOTICE). Distributed here under AGPL-3.0-only;
//   BSD-3-Clause -> (A)GPLv3 is one-way.
// ---------------------------------------------------------------------------

//! **SMT's kriging family** (gh:#1190, epic gh:#1177): the second kriging
//! route beside RAVEN/sklearn's ([`crate::surrogate::gaussian_process`]).
//!
//! | SMT | Here |
//! |---|---|
//! | `KRG` | [`SmtKriging`] with [`SmtPls::None`] |
//! | `KPLS` | [`SmtKriging`] with [`SmtPls::Kpls`] (PLS from scikit-learn, [`pls`]) |
//! | `KPLSK` | [`SmtKriging`] with [`SmtPls::Kplsk`] |
//! | `MFK` | [`Mfk`] |
//!
//! SMT's defaults are [`SmtKrgConfig::smt_defaults`] and
//! [`MfkConfig::smt_defaults`]: `squar_exp`, constant trend, nugget `100 ε`,
//! θ0 `1e-2`, θ bounds `[1e-6, 20]`, X and y standardised with the sample
//! std, 10-point multistart. **The optimiser is not SMT's** (TNC is not
//! ported): it is this crate's bounded BFGS on SMT's own objective, so fits
//! agree at optima, not along paths. Typed access goes through
//! `fidelity::SurrogateSpec::Kriging { route: KrigingRoute::Smt(..) }` and
//! `SurrogateSpec::CoKriging`.
//!
//! # V&V
//!
//! Record: `crates/raffles/verification_and_validation/smt_kriging_gh1190.md`
//! (predictions committed before this code); differential tests against SMT
//! in `tests/smt_kriging_gh1190.rs`.

pub mod core;
pub mod krg;
pub mod linalg;
pub mod mfk;
pub mod pls;

pub use self::core::{SmtCorrelation, SmtOptimizer, SmtRegression, SmtStarts};
pub use krg::{SmtKriging, SmtKrgConfig, SmtPls, TrendVariance};
pub use mfk::{Mfk, MfkConfig};
