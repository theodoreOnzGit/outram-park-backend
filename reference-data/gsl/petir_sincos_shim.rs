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

// gcc -O2 fuses `sin(x)` and `cos(x)` on the same argument (qmomof.c,
// qc25f.c) into one `sincos` call, which a shim of `sin` and `cos` alone
// would miss. Evaluated separately, as GSL's source writes them.
#[no_mangle]
pub extern "C" fn sincos(x: f64, s: *mut f64, c: *mut f64) {
    // SAFETY: the C caller passes two valid, writable doubles.
    unsafe {
        *s = Real::sin(x);
        *c = Real::cos(x);
    }
}
