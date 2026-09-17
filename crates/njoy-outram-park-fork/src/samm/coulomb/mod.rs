//! Coulomb wave functions for charged-particle channels — ported from
//! `samm.f90`'s Coulomb library: `jwkb`, `coulfg` (Steed's method, the CPC
//! "COULFG" algorithm of Barnett), `xsigll`, `asymp1`, `asymp2`, `taylor`,
//! `end1`, `getfg`, `bigeta`, `getps`, `coulx`, `pspcou`, `pghcou`.
//!
//! This is the charged-particle counterpart of [`super::penetrability`]:
//! where that module's `pgh` gives the penetrability/shift/phase-shift for
//! uncharged (neutral) channels via closed-form hard-sphere formulas, this
//! module's [`pghcou`] gives the same three quantities for channels where
//! the Coulomb interaction between the two exit particles matters (charged
//! exit channels — e.g. `(n,p)`, `(n,alpha)` on light nuclides).
//!
//! Split across four files, one per functional group (none over ~400
//! lines): [`steed`] (Steed's method core: `jwkb`, `coulfg`), [`asymptotic`]
//! (the moderate-`rho` asymptotic-expansion family: `xsigll`, `asymp1`,
//! `asymp2`, `taylor`, `end1`, `getfg`), [`dispatch`] (the large-`eta`
//! Bessel-function case and the top-level strategy dispatcher: `bigeta`,
//! `getps`, `coulx`), and [`api`] (the two entry points other modules
//! actually call: `pspcou`, `pghcou`).
//!
//! # Indexing convention
//!
//! Every Fortran array here (`fc`/`gc`/`fcp`/`gcp` in `coulfg`; `f`/`fpr`/
//! `g`/`gpr` elsewhere) is 1-indexed with array position `p` holding the
//! quantity at orbital angular momentum `L = p-1` (`samm.f90`'s own
//! convention, since `Xlm`/`Llmin` is always `0` in this port — see
//! [`steed::coulfg`]'s doc comment). This port uses **direct
//! 0-indexed-by-`L`** `Vec<f64>`s throughout instead (`vec[L]` rather than
//! `array(L+1)`), which is the natural Rust mapping and removes a constant
//! `+1`/`-1` from every access site — but every derivation is still
//! checked position-by-position against the original Fortran, not
//! re-derived from scratch, precisely because an indexing slip here is
//! easy to make and hard to notice.

pub mod api;
pub mod asymptotic;
pub mod dispatch;
pub mod steed;

pub use api::{pghcou, pspcou, Pghcou};
pub use asymptotic::{asymp1, asymp2, end1, getfg, taylor, xsigll};
pub use dispatch::{bigeta, coulx, getps, CoulombPsp};
pub use steed::{coulfg, jwkb, CoulfgResult, Jwkb};
