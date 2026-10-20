// Shim: export the `libm` crate's (musl-derived) `hypot` under the C name,
// so a GSL reference driver linked against it uses exactly the `hypot` that
// PETIR's `crate::real::Real::hypot` uses. Linked ahead of glibc's `-lm`.
//
// Purpose (gh:#993): isolate a platform-libm last-ulp difference from a
// porting defect. If PETIR matches GSL-with-this-shim bit for bit, the whole
// difference from stock GSL is `hypot`, not the port. Build recipe in
// README.md ("Isolating `hypot`").
#![no_std]

#[no_mangle]
pub extern "C" fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

// The prebuilt `core` in the rlib's dependency graph references this symbol;
// nothing here unwinds, so an empty definition satisfies the linker.
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
