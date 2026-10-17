// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Runtime gate: `process` and `NuclearDataLibrary` RUN in a wasm runtime
//! and give the native numbers** (epic #925's wasm criterion; #928, #933).
//!
//! # Methodology
//!
//! - **Input:** ENDF/B-VIII.0 F-19 (MAT 925), compiled into the test with
//!   `include_bytes!` so neither run reads a file: bytes in, values out.
//! - **Two paths, one fingerprint.** `process::cross_sections` at 293.6 K
//!   with the default settings (RECONR, BROADR, HEATR, the 0 K elastic; F-19
//!   has no unresolved range, so PURR does nothing), then ACER; and
//!   `NuclearDataLibrary::from_bytes(..).reconstruct(0.001).broaden(293.6 K)`
//!   then `to_ace`. The fingerprint is FNV-1a over the bits of every PENDF
//!   energy and cross section, the 0 K elastic, and the ACE table's NXS, JXS
//!   and XSS words (not its header, whose `hk` field carries the build's git
//!   commit). Pass: the two paths agree, and both equal the fingerprint the
//!   native run printed, pinned below.
//! - **Runs:** natively (`cargo test --release`), and in a wasm runtime:
//!   `cargo test --release --target wasm32-wasip1 -p njoy-outram-park-fork
//!   --test wasm_process_runtime`, under Node's WASI through
//!   `scripts/wasi-run.mjs`. That runtime has no threads, like a browser, so
//!   it exercises the serial fallbacks; it has a clock and a file system,
//!   unlike a browser, so it cannot show that nothing here reads either. The
//!   browser check (headless Chromium, #925) was **not** run for this change.
//!
//! # Results (2026-10-17)
//!
//! - **Native** (x86_64 Linux, i9-13900K, release): fingerprint
//!   `0x84fba8cfa826c2da` over 175 194 XSS words; object and free function
//!   agree. 0.34 s.
//! - **wasm32-wasip1** (Node 26.11.1 WASI, release): the **same**
//!   fingerprint, `0x84fba8cfa826c2da`; object and free function agree.
//!   0.46 s. So on this nuclide RECONR, BROADR, HEATR, ACER, the 0 K elastic
//!   and the object layer give the native numbers to the bit in a wasm
//!   runtime.
//!
//! Running it: a worktree nested inside the main checkout picks up the
//! runner from both `.cargo/config.toml` files (Cargo concatenates the
//! arrays), and `cargo test --target wasm32-wasip1` then fails with
//! `ENOENT ... 'bash'`. Build with that command and run the `.wasm` with
//! `node --no-warnings scripts/wasi-run.mjs <file>.wasm` from the crate
//! directory instead.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::interface::NuclearDataLibrary;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, NuclearCrossSections};
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

const F19: &[u8] = include_bytes!("../../../reference-data/endf/n-009_F_019-ENDF8.0.endf");

/// The fingerprint the native run printed (x86_64 Linux, release,
/// 2026-10-17). A wasm run that differs has different numbers, not merely a
/// different build.
const NATIVE_FINGERPRINT: u64 = 0x84fb_a8cf_a826_c2da;

/// FNV-1a over 64-bit words.
fn fnv(words: impl Iterator<Item = u64>, mut h: u64) -> u64 {
    for w in words {
        for b in w.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn fingerprint(xs: &NuclearCrossSections) -> (u64, usize) {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for s in &xs.pendf.sections {
        h = fnv(
            s.pairs
                .iter()
                .flat_map(|&(e, v)| [e.to_bits(), v.to_bits()]),
            h,
        );
    }
    let el0 = xs.elastic_0k.as_deref().expect("0 K elastic on by default");
    h = fnv(el0.iter().flat_map(|&(e, v)| [e.to_bits(), v.to_bits()]), h);
    let ace = xs.to_ace().expect("ACER");
    h = fnv(ace.nxs.iter().chain(ace.jxs.iter()).map(|&i| i as u64), h);
    h = fnv(ace.xss.iter().map(|x| x.to_bits()), h);
    (h, ace.xss.len())
}

#[test]
fn process_and_the_object_run_and_give_the_native_numbers() {
    let t = Temperature::new::<kelvin>(293.6);
    let tape = Tape::read(F19).expect("parse F-19");
    let free =
        process::cross_sections(&tape, t, &CrossSectionSettings::default()).expect("process");
    let obj = NuclearDataLibrary::from_bytes(F19)
        .expect("from_bytes")
        .reconstruct(1.0e-3)
        .expect("RECONR")
        .broaden(t)
        .expect("BROADR");
    let (h_free, n_xss) = fingerprint(&free);
    let (h_obj, _) = fingerprint(obj.cross_sections().expect("processed"));
    println!(
        "F-19 293.6 K: fingerprint {h_free:#018x}, {n_xss} XSS words, target {}",
        std::env::consts::ARCH
    );
    assert_eq!(h_obj, h_free, "the object and the free function disagree");
    assert_eq!(
        h_free, NATIVE_FINGERPRINT,
        "this runtime's numbers differ from the native run's"
    );
}
