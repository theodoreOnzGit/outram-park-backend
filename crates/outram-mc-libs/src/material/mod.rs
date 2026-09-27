//! Materials and the nuclear data behind them.
//!
//! - [`nuclide`]: one nuclide's cross sections and secondary-particle laws
//!   ([`Nuclide`](nuclide::Nuclide)), from embedded, ENDF, ACE or HDF5 data.
//! - [`material`]: a mixture of nuclides at atom densities, and the
//!   macroscopic cross sections of that mixture.
//! - [`thermal`]: bound-atom S(alpha,beta) thermal scattering.
//! - [`reaction`]: reaction identifiers used across the above.
//! - [`speed`]: [`SpeedTier`](speed::SpeedTier), trading accuracy for speed in
//!   nuclear-data processing and cross-section lookup.
//!
//! Geometry, sources and tallies live in their own modules; transport itself
//! is in [`crate::physics`].
pub mod material;
pub mod nuclide;
pub mod reaction;
pub mod speed;
pub mod thermal;
