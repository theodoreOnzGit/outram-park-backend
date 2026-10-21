//! **The nuclear-data loading probe** (gh #935, #936): a `cdylib` whose
//! exports let a web page hand files to the same Rust that runs natively,
//! and read back fingerprints. It is a test harness, not a demo page: the
//! browser record in
//! `verification_and_validation/nuclear_data_loading_web/` drives it in
//! headless Chromium and compares with the native run of the same functions
//! (`tests/nd_wasm_probe_native.rs`).
//!
//! ```text
//! cargo build --release --target wasm32-unknown-unknown -p outram-mc-libs --example nd_wasm_probe
//! wasm-bindgen --target web --no-typescript --out-dir <dir> \
//!     target/wasm32-unknown-unknown/release/examples/nd_wasm_probe.wasm
//! ```
//!
//! The exports are plain `extern "C"` functions with no `unsafe`: a buffer
//! is staged with `probe_alloc` (the page writes into the returned pointer),
//! `probe_add_file` turns the last two staged buffers into a `(name, bytes)`
//! file, `probe_run` runs a task, and `probe_out_ptr`/`probe_out_len` give
//! its report. wasm-bindgen only supplies the glue the dependency tree needs
//! (getrandom's `wasm_js` backend); nothing here is `#[wasm_bindgen]`.
//!
//! Native builds compile it as a shared library that nothing loads (the
//! exports need a host); its logic is reached natively through
//! `tests/nd_wasm_probe_native.rs`, which includes `probe.rs`.

#[path = "probe.rs"]
pub mod probe;

use std::cell::RefCell;

thread_local! {
    static STAGED: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    static FILES: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    static ZIP: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Stage a zeroed buffer of `len` bytes and return where it is; the page
/// writes the bytes there. The buffer's heap allocation does not move when
/// the `Vec` is stored.
#[no_mangle]
pub extern "C" fn probe_alloc(len: usize) -> *mut u8 {
    let mut v = vec![0u8; len];
    let p = v.as_mut_ptr();
    STAGED.with(|s| s.borrow_mut().push(v));
    p
}

/// Turn the last two staged buffers (name, then contents) into a file.
/// Returns 0, or 1 when fewer than two are staged.
#[no_mangle]
pub extern "C" fn probe_add_file() -> u32 {
    let pair = STAGED.with(|s| {
        let mut s = s.borrow_mut();
        let data = s.pop()?;
        let name = s.pop()?;
        Some((String::from_utf8_lossy(&name).into_owned(), data))
    });
    match pair {
        Some(f) => {
            FILES.with(|fs| fs.borrow_mut().push(f));
            0
        }
        None => 1,
    }
}

/// Run task `task`: 1 zip the files and index the zip; 2 generate F-19's
/// ACE from that zip; 3 Godiva from that zip; 4 index and decode the files
/// as ACE (they are then cleared). Returns 0 on success, 1 on error; the
/// report or the error is in the output buffer.
#[no_mangle]
pub extern "C" fn probe_run(task: u32) -> u32 {
    let result = match task {
        1 => FILES
            .with(|f| probe::zip_and_index(&f.borrow()))
            .map(|(zip, report)| {
                ZIP.with(|z| *z.borrow_mut() = zip);
                report
            }),
        2 => ZIP.with(|z| probe::generate_f19(&z.borrow())),
        3 => ZIP.with(|z| probe::godiva_from_zip(&z.borrow())),
        4 => probe::ace_tables(FILES.with(|f| std::mem::take(&mut *f.borrow_mut()))),
        _ => Err(format!("no task {task}")),
    };
    let (code, text) = match result {
        Ok(r) => (0, r),
        Err(e) => (1, format!("error={e}\n")),
    };
    OUT.with(|o| *o.borrow_mut() = text);
    code
}

/// Forget every file and staged buffer.
#[no_mangle]
pub extern "C" fn probe_clear() {
    STAGED.with(|s| s.borrow_mut().clear());
    FILES.with(|f| f.borrow_mut().clear());
}

/// Where the last report is.
#[no_mangle]
pub extern "C" fn probe_out_ptr() -> *const u8 {
    OUT.with(|o| o.borrow().as_ptr())
}

/// How long the last report is, in bytes.
#[no_mangle]
pub extern "C" fn probe_out_len() -> usize {
    OUT.with(|o| o.borrow().len())
}
