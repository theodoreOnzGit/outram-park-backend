// Diagnostic shim (gh:#1340 round 3): export PETIR's `sin` and `cos`
// (crate::real::Real, the `libm` crate's code) under the C names, so a GSL
// reference driver linked against it uses exactly the weights PETIR's QAWO /
// QAWF use. Linked ahead of glibc's `-lm`; the result is a diagnostic, never
// committed as a reference. A `std` staticlib, because petir links `alloc`.
use petir::real::Real;

#[no_mangle]
pub extern "C" fn sin(x: f64) -> f64 {
    Real::sin(x)
}
#[no_mangle]
pub extern "C" fn cos(x: f64) -> f64 {
    Real::cos(x)
}
