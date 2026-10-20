// Shim: export PETIR's CORE-MATH `hypot`/`hypotf` (crate::hypot) and the
// `libm` crate's (musl) `hypot`/`hypotf` under C names, so
// `hypot_mpfr_driver.c` can check every input of its sweep against MPFR
// one by one (gh:#997). Build recipe in README.md.
// Built as a `std` staticlib: petir links `alloc`, which needs an allocator.

#[no_mangle]
pub extern "C" fn petir_hypot(x: f64, y: f64) -> f64 {
    petir::hypot::hypot(x, y)
}

#[no_mangle]
pub extern "C" fn petir_hypotf(x: f32, y: f32) -> f32 {
    petir::hypot::hypotf(x, y)
}

#[no_mangle]
pub extern "C" fn musl_hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

#[no_mangle]
pub extern "C" fn musl_hypotf(x: f32, y: f32) -> f32 {
    libm::hypotf(x, y)
}
