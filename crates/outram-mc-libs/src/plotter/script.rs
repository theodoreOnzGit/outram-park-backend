// SPDX-License-Identifier: GPL-3.0-only
//! **Emitting a standalone matplotlib script.**
//!
//! The ports in [`super::xs`] and [`super::tracks`] compute, in Rust, exactly
//! the arrays OpenMC's Python functions hand to matplotlib, and record the
//! axis calls they make. This module writes those down as a `.py` file that
//! makes the same calls on the same arrays, so that running it draws the
//! figure OpenMC would have drawn.
//!
//! # Why the data is embedded as bytes and not as decimal literals
//!
//! Pixel identity needs bit identity of every float. A float64 written as the
//! shortest round-trip decimal does survive a Python parse exactly, but it is
//! 18-24 characters per value; a U-235 grid is 76 027 points per line. The
//! arrays are therefore embedded as little-endian IEEE-754 float64, zlib
//! compressed (level 9) and base64 encoded, and decoded with
//! `np.frombuffer(zlib.decompress(base64.b64decode(s)), dtype='<f8')`. The
//! round trip is exact by construction. Identical arrays (the energy grid is
//! shared by every line of a nuclide) are stored once.
//!
//! The script needs only `numpy` and `matplotlib`, and runs as
//! `python3 script.py [out.png]`. `build_figure()` returns the figure, so a
//! caller can also import the script and inspect `fig.axes[0].lines`.

use std::collections::HashMap;

/// Keyword arguments to `plt.subplots(**kwargs)` / `plt.figure(**kwargs)`.
///
/// Only the two that change the rendered image are carried.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FigureKwargs {
    /// `figsize=(w, h)` \[inch\].
    pub figsize: Option<(f64, f64)>,
    /// `dpi=`.
    pub dpi: Option<f64>,
}

impl FigureKwargs {
    /// The argument list, e.g. `figsize=(8.0, 6.0), dpi=100.0`, or empty.
    pub fn to_kwargs(&self) -> String {
        let mut v = Vec::new();
        if let Some((w, h)) = self.figsize {
            v.push(format!("figsize=({}, {})", py_float(w), py_float(h)));
        }
        if let Some(d) = self.dpi {
            v.push(format!("dpi={}", py_float(d)));
        }
        v.join(", ")
    }
}

/// A Python string literal for `s` (single-quoted, escaped; non-ASCII as
/// `\uXXXX`/`\UXXXXXXXX` so the file is pure ASCII).
pub fn py_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('\'');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\'' => o.push_str("\\'"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c if c.is_ascii() => o.push(c),
            c if (c as u32) <= 0xffff => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push_str(&format!("\\U{:08x}", c as u32)),
        }
    }
    o.push('\'');
    o
}

/// A Python float literal that parses back to exactly `v`.
///
/// Rust's `{:?}` prints the shortest string that round-trips, as Python's
/// `repr` does; the spelling can differ (`1e-5` against `1e-05`) but both
/// parse to the same double.
pub fn py_float(v: f64) -> String {
    if v.is_nan() {
        "float('nan')".into()
    } else if v == f64::INFINITY {
        "float('inf')".into()
    } else if v == f64::NEG_INFINITY {
        "float('-inf')".into()
    } else {
        format!("{v:?}")
    }
}

/// Standard base64 with padding (RFC 4648 section 4), as `base64.b64decode`
/// expects.
pub fn base64_encode(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for ch in bytes.chunks(3) {
        let b = [ch[0], *ch.get(1).unwrap_or(&0), *ch.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        o.push(A[(n >> 18) as usize & 63] as char);
        o.push(A[(n >> 12) as usize & 63] as char);
        o.push(if ch.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if ch.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    o
}

/// Encode a float64 array the way the emitted script decodes it.
pub fn encode_f64(v: &[f64]) -> String {
    let mut raw = Vec::with_capacity(v.len() * 8);
    for x in v {
        raw.extend_from_slice(&x.to_le_bytes());
    }
    base64_encode(&miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9))
}

/// FNV-1a 64-bit hash of the little-endian float64 bytes of `v`.
///
/// A dependency-free fingerprint both Rust and Python compute identically
/// (the V&V record stores it for every reference line), so bit identity of
/// the plotted data can be checked without Python.
pub fn fnv1a64_f64(v: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in v {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// A script under construction: an array pool plus the body of
/// `build_figure()`.
#[derive(Debug, Clone)]
pub struct PyScript {
    producer: String,
    upstream: String,
    arrays: Vec<String>,
    index: HashMap<Vec<u64>, usize>,
    body: Vec<String>,
}

impl PyScript {
    /// Start a script. `producer` is the Rust function, `upstream` the OpenMC
    /// function it ports; both go in the header.
    pub fn new(producer: &str, upstream: &str) -> Self {
        Self {
            producer: producer.into(),
            upstream: upstream.into(),
            arrays: Vec::new(),
            index: HashMap::new(),
            body: Vec::new(),
        }
    }

    /// Add an array (deduplicated by exact bit pattern); returns its index in
    /// `a`.
    pub fn array(&mut self, v: &[f64]) -> usize {
        let key: Vec<u64> = v.iter().map(|x| x.to_bits()).collect();
        if let Some(&i) = self.index.get(&key) {
            return i;
        }
        let i = self.arrays.len();
        self.arrays.push(encode_f64(v));
        self.index.insert(key, i);
        i
    }

    /// Append one line to `build_figure()`.
    pub fn line(&mut self, code: &str) {
        self.body.push(code.to_string());
    }

    /// The finished script. `default_png` is written when no path is given on
    /// the command line.
    pub fn finish(&self, default_png: &str) -> String {
        let mut s = String::new();
        s.push_str("#!/usr/bin/env python3\n");
        s.push_str("# SPDX-License-Identifier: GPL-3.0-only\n");
        s.push_str(&format!(
            "# Generated by outram-mc-libs {} `{}`,\n",
            env!("CARGO_PKG_VERSION"),
            self.producer
        ));
        s.push_str(&format!(
            "# a Rust port of OpenMC's {} at OpenMC commit d7d3284a1\n",
            self.upstream
        ));
        s.push_str("# (0.16.1.dev25; OpenMC is MIT-licensed). Standalone: the plotted arrays are\n");
        s.push_str("# embedded below as zlib+base64 little-endian float64; needs numpy + matplotlib.\n");
        s.push_str("# Usage: python3 this_script.py [output.png]\n");
        s.push_str("import base64\nimport sys\nimport zlib\n\n");
        s.push_str("import numpy as np\nimport matplotlib.pyplot as plt\n\n");
        s.push_str("_ARRAYS = [\n");
        for a in &self.arrays {
            s.push_str("    '");
            s.push_str(a);
            s.push_str("',\n");
        }
        s.push_str("]\n\n\n");
        s.push_str("def _array(i):\n");
        s.push_str(
            "    return np.frombuffer(zlib.decompress(base64.b64decode(_ARRAYS[i])), dtype='<f8').copy()\n\n\n",
        );
        s.push_str("def build_figure():\n");
        s.push_str("    a = [_array(i) for i in range(len(_ARRAYS))]\n");
        for l in &self.body {
            s.push_str("    ");
            s.push_str(l);
            s.push('\n');
        }
        s.push_str("    return fig\n\n\n");
        s.push_str("if __name__ == '__main__':\n");
        s.push_str(&format!(
            "    build_figure().savefig(sys.argv[1] if len(sys.argv) > 1 else {})\n",
            py_str(default_png)
        ));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn python_literals() {
        assert_eq!(py_str("U235 (n,2n) U234"), "'U235 (n,2n) U234'");
        assert_eq!(py_str("it's"), "'it\\'s'");
        assert_eq!(py_float(1.0e-5), "1e-5");
        assert_eq!(py_float(2.0e7), "20000000.0");
    }
}
