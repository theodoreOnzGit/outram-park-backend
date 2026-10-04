//! ENDF tape bytes: what is stripped before download, how it travels, and
//! how it is read back. Shared by every rung (moved from the TRISO rung's
//! `model.rs`, 2026-10-04, gh:#521).

// Data preparation is native-only; decompress is browser-only.
#![allow(dead_code)]

use njoy_outram_park_fork::endf::tape::Tape;

/// Drop the covariance files (MF 30-40) from an ENDF tape.
///
/// Covariances are uncertainty data for ERRORR; transport never reads them,
/// yet they are most of the bytes — 39 of O-16's 42 MB, about 26 of U-235's
/// 37 MB. Removing them is what makes a browser download tolerable. That this
/// changes NOTHING the transport sees is not assumed: the example's tests
/// build nuclides from the full and the stripped tapes and require
/// bit-identical cross sections and fission-spectrum samples.
///
/// A covariance file ends with an FEND record (MF = 0); that record is dropped
/// with it, so the file structure stays well formed.
pub fn strip_covariances(tape: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(tape.len());
    let mut in_cov = false;
    for line in tape.split_inclusive(|&b| b == b'\n') {
        let mf = std::str::from_utf8(line.get(70..72).unwrap_or(b""))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        match mf {
            Some(30..=40) => in_cov = true,
            Some(0) if in_cov => in_cov = false,
            _ => out.extend_from_slice(line),
        }
    }
    out
}

pub fn read_tape(bytes: &[u8], what: &str) -> Result<(Tape, i32), String> {
    let tape = Tape::read(std::io::Cursor::new(bytes)).map_err(|e| format!("{what}: {e}"))?;
    let mat = *tape.materials().first().ok_or_else(|| format!("{what}: no material on tape"))?;
    Ok((tape, mat))
}


// ─── Tape bytes on the wire ──────────────────────────────────────────────────

/// What the browser downloads: the covariance-stripped tape, zlib-compressed.
pub fn compress(stripped: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(stripped, 9)
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // only the browser inflates
pub fn decompress(zlib: &[u8]) -> Result<Vec<u8>, String> {
    miniz_oxide::inflate::decompress_to_vec_zlib(zlib).map_err(|e| format!("inflate: {e:?}"))
}

/// The file name a tape is served under.
pub fn wire_name(tape: &str) -> String {
    format!("{tape}.zz")
}
