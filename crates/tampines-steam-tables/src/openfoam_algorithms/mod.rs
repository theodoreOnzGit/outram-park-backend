//! Pure-Rust ports of OpenFOAM finite-volume solver algorithms, used as
//! reference/study material and (for `rhoPimpleFoam`) as an actively-used
//! solver. Most submodules here are `mod`/`pub(crate)` — kept in-tree for
//! their FV building blocks (see `openfoam_source`) rather than exposed as
//! part of this crate's public API. The sole public export is
//! [`rhoPimpleFoam`], which hosts `TampinesSteamArray`, a 1-D compressible
//! PIMPLE pipe solver built on TAMPINES IAPWS-IF97 steam-table properties.
#[allow(non_snake_case)]
mod simplefoam;

// apparently for two phase flow,
// this is a useful algorithm to learn from.
//
// https://www.tfd.chalmers.se/~hani/kurser/OS_CFD_2008/PraveenPrabhuBaila/Report_twoPhaseEuler.pdf
//
// though for a first model, it may be too complex for a first model
#[allow(non_snake_case)]
mod chtMultiRegionTwoPhaseEulerFoam;

// a less complicated model is drift flux
// drift flux model
#[allow(non_snake_case)]
mod driftFluxFoam;

// also notable is the VOF methods
// https://www.cfd-online.com/Forums/openfoam-solving/58063-vof-method.html
// which interfoam uses
//
// this helps it to find an interface between the fluid and gas
//
//
// This is interphasechangefoam
// https://www.tfd.chalmers.se/~hani/kurser/OS_CFD_2011/MartinAndersen/Tutorial_interPhaseChangeFoam.pdf
//
// apparently accounts for boiling
//
//
// Some other useful tutorials here:
// https://www.wolfdynamics.com/training/mphase/OF2021/mphase_2021_OF8_guided_tutorials.pdf
//
// it seems the most suitable model is two phase euler foam,
//
// where averaged equations are used to describe both phases.
//
// https://www.tfd.chalmers.se/~hani/kurser/OS_CFD_2008/PraveenPrabhuBaila/Report_twoPhaseEuler.pdf
//
//
// for the two phase Euler Foam, the PhD thesis most helpful is:
//
// https://spiral.imperial.ac.uk/entities/publication/3f1ca3a6-f78e-48d5-a928-cf94b2292bd0
//
// Rusche, H. (2002). Computational fluid dynamics of dispersed two-phase
// flow at high phase fractions. Ph. D. thesis, University of London.
//

//
// rhoPimpleFoam
// pimple algorithm for compressible flow
// rust code for rhoPimpleFoam
// is also here
/// Rust port of OpenFOAM's `rhoPimpleFoam` (compressible PIMPLE) solver,
/// specialised into `TampinesSteamArray` — a 1-D pipe solver over TAMPINES
/// IAPWS-IF97 steam properties. This is the only public export of
/// `openfoam_algorithms`.
#[allow(non_snake_case)]
pub mod rhoPimpleFoam;

pub(crate) mod openfoam_source;
