// Shim (gh:#1338): export PETIR's own transcendental functions under the C
// names GSL's specfunc calls, so a GSL reference driver linked against it
// uses exactly the `exp`, `log`, `pow`, `sin`, ... that PETIR's specfunc
// uses (`crate::real::Real`: the ARM optimized-routines ports for exp/log/pow,
// the `libm` crate's musl code for the rest, and the CORE-MATH `hypot`).
// Linked ahead of glibc's `-lm`; see README.md, "Isolating libm". Successor
// of `petir_hypot_shim.rs`. A `std` staticlib, because petir links `alloc`.
use petir::real::Real;

#[no_mangle]
pub extern "C" fn exp(x: f64) -> f64 {
    Real::exp(x)
}
#[no_mangle]
pub extern "C" fn log(x: f64) -> f64 {
    Real::ln(x)
}
#[no_mangle]
pub extern "C" fn pow(x: f64, y: f64) -> f64 {
    Real::powf(x, y)
}
#[no_mangle]
pub extern "C" fn sin(x: f64) -> f64 {
    Real::sin(x)
}
#[no_mangle]
pub extern "C" fn cos(x: f64) -> f64 {
    Real::cos(x)
}
#[no_mangle]
pub extern "C" fn tan(x: f64) -> f64 {
    Real::tan(x)
}
#[no_mangle]
pub extern "C" fn sinh(x: f64) -> f64 {
    Real::sinh(x)
}
#[no_mangle]
pub extern "C" fn cosh(x: f64) -> f64 {
    Real::cosh(x)
}
#[no_mangle]
pub extern "C" fn tanh(x: f64) -> f64 {
    Real::tanh(x)
}
#[no_mangle]
pub extern "C" fn atan(x: f64) -> f64 {
    Real::atan(x)
}
#[no_mangle]
pub extern "C" fn atan2(y: f64, x: f64) -> f64 {
    Real::atan2(y, x)
}
#[no_mangle]
pub extern "C" fn asin(x: f64) -> f64 {
    Real::asin(x)
}
#[no_mangle]
pub extern "C" fn acos(x: f64) -> f64 {
    Real::acos(x)
}
#[no_mangle]
pub extern "C" fn expm1(x: f64) -> f64 {
    Real::exp_m1(x)
}
#[no_mangle]
pub extern "C" fn log1p(x: f64) -> f64 {
    Real::ln_1p(x)
}
#[no_mangle]
pub extern "C" fn hypot(x: f64, y: f64) -> f64 {
    petir::hypot::hypot(x, y)
}
// Added 2026-11-12 (gh:#1421). `gcc -O2` rewrites a `sin(y)` and `cos(y)` of
// the same argument into ONE call to glibc's `sincos`, which GSL's libgsl.a
// references 17 times (`nm libgsl.a | grep ' U sincos'`) and which this shim did
// not export, so those calls still reached glibc: bessel J/Y's `cos_pi4` /
// `sin_pi4` differed by an ulp for that reason alone.
/// # Safety
/// `s` and `c` must be valid for writes, as for C's `sincos`.
#[no_mangle]
pub unsafe extern "C" fn sincos(x: f64, s: *mut f64, c: *mut f64) {
    *s = Real::sin(x);
    *c = Real::cos(x);
}
