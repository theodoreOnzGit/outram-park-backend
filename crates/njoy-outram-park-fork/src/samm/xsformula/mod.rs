//! Cross-section evaluation formula — Phase 5 of the `samm` port. Ported
//! from `samm.f90`'s `abpart`, `setr`, `setxqx`, `sectio` (the
//! non-derivative, non-angular core of each).
//!
//! Split by function per this crate's file-size convention:
//! [`abpart`] (Breit-Wigner denominator terms), [`setr`] (R-matrix/level-
//! matrix assembly at one energy), [`assembly`] (`setxqx`'s `XQ`/`XXXX`
//! matrices), [`sectio`] (cross-section pieces from `XXXX`), and
//! [`crosss`] (the top-level per-energy dispatcher summing every spin
//! group — the actual entry point).
//!
//! **Not ported (deferred until built alongside `ERRORR`, their real
//! caller — see `README.md`):** `babb`/`abpart`'s and `setr`'s derivative
//! branches, `setqri`/`settri`/`gcphase` (derivative and angular-
//! distribution assembly), `derres`/`derext` (derivative propagation).

pub mod abpart;
pub mod assembly;
pub mod crosss;
pub mod sectio;
pub mod setr;

pub use crosss::cross_sections;
