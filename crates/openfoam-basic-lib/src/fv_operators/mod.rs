/// Explicit finite-volume operators — return a new field.
///
/// Usage mirrors `Foam::fvc::` from `src/finiteVolume/finiteVolume/fvc/`.
pub mod fvc;

/// Implicit finite-volume operators — assemble into a sparse `FvMatrix`.
///
/// Usage mirrors `Foam::fvm::` from `src/finiteVolume/finiteVolume/fvm/`.
pub mod fvm;
