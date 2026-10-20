// Shim: export PETIR's `hypot` (crate::hypot, the CORE-MATH port, gh:#997)
// under the C name, so a GSL reference driver linked against it uses exactly
// the `hypot` PETIR uses. Linked ahead of glibc's `-lm`. Successor of
// `libm_shim.rs` (gh:#993), which did the same for the `libm` crate's
// `hypot` that PETIR used before. Build recipe in README.md ("Isolating
// `hypot`"). A `std` staticlib, because petir links `alloc`.

#[no_mangle]
pub extern "C" fn hypot(x: f64, y: f64) -> f64 {
    petir::hypot::hypot(x, y)
}
