// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! `fnv_tree <dir>`: print `relative/path fnv1a64-hex` for every file under `<dir>` whose first
//! path component starts with `processor`, sorted. Used to freeze the ESI v2506 `decomposePar`
//! outputs of gh:#1381 as hashes (`tests/fixtures/esi_decompose/fnv_<case>_<variant>.txt`).
//!
//! CLI glue (no test reaches `main`); the same hash is computed in
//! `tests/decompose_par_esi_v2506.rs`, which compares it with the frozen table.

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, u64)>) {
    for e in std::fs::read_dir(dir).expect("read_dir") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            walk(base, &p, out);
        } else {
            let rel = p.strip_prefix(base).expect("prefix").to_string_lossy().replace('\\', "/");
            out.push((rel, fnv(&std::fs::read(&p).expect("read"))));
        }
    }
}

fn main() {
    let base = std::path::PathBuf::from(std::env::args().nth(1).expect("dir"));
    let mut v = Vec::new();
    for e in std::fs::read_dir(&base).expect("read_dir") {
        let p = e.expect("entry").path();
        if p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("processor")) {
            walk(&base, &p, &mut v);
        }
    }
    v.sort();
    for (rel, h) in v {
        println!("{rel} {h:016x}");
    }
}
