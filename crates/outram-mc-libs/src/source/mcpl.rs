// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Two upstreams are ported here.
//
// 1. MCPL (https://mctools.github.io/mcpl/), mcpl-core 2.2.8, src/mcpl.c:
//    mcpl_write_header, mcpl_internal_serialise_particle_to_buffer,
//    mcpl_unitvect_pack_adaptproj / _unpack_adaptproj / _unpack_oct,
//    mcpl_actual_open_file, mcpl_read, mcpl_internal_encodestatsum,
//    mcpl_internal_statsumparse, mcpl_create_outfile (file-name rule).
//    Copyright 2015-2026 MCPL developers. Licensed under the Apache License,
//    Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0); Apache-2.0 is
//    compatible with the GPLv3/AGPLv3 this crate is conveyed under. Changes:
//    translated to Rust over an in-memory byte buffer; no C library is used.
//
// 2. OpenMC (https://github.com/openmc-dev/openmc), commit
//    8f79d9d02483c90e4134f03a9df4b75ce6b062bf, src/mcpl_interface.cpp:
//    mcpl_particle_to_site, mcpl_source_sites, write_mcpl_source_point,
//    write_mcpl_collision_track(_internal).
//    Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
//    Argonne LLC, and OpenMC contributors. MIT licence (full text in this
//    crate's LICENSE.openmc).

//! **MCPL particle files, read and written in pure Rust** (gh:#1392).
//!
//! MCPL ("Monte Carlo Particle Lists") is the interchange format OpenMC shares
//! with Geant4, McStas, McXtrace and PHITS. OpenMC reaches it through
//! `src/mcpl_interface.cpp`, which `dlopen`s the C library `libmcpl` at run
//! time. This module ports **the format itself** from `mcpl.c` instead, so it
//! builds for `wasm32` and Android with no C dependency, and then ports
//! OpenMC's three uses of it on top:
//!
//! | OpenMC | here |
//! |---|---|
//! | `mcpl_source_sites` (a source `<file>` ending `.mcpl`) | [`mcpl_source_sites`] |
//! | `write_mcpl_source_point` (`<source_point mcpl="true">`, surface source) | [`write_mcpl_source_point`] |
//! | `write_mcpl_collision_track` (`<collision_track><mcpl>`) | [`write_mcpl_collision_track`] |
//!
//! # The format (MCPL-3, as `mcpl.c` writes it)
//!
//! ```text
//! "MCPL" "003" 'L'                      8 bytes: magic, version, endianness
//! u64 nparticles
//! u32 ncomments, nblobs, userflags, polarisation, singleprec,
//!     universal_pdgcode, particle_size, has_universal_weight
//! [f64 universal_weight]
//! string srcname; string comment * ncomments
//! string blobkey * nblobs; buffer blob * nblobs     (u32 length + bytes)
//! particle * nparticles, each particle_size bytes:
//!   [pol x3] pos x3  (dir packed with ekin) x3  time  [weight]  [i32 pdg]  [u32 flags]
//!   in f32 when singleprec, else f64
//! ```
//!
//! The direction uses Kittelmann's *adaptive projection packing*: the two
//! smallest components are stored, the largest one as its inverse in their
//! place when it is not `z`, and the sign of the dropped component is carried
//! by the sign bit of the kinetic energy. MCPL-2 files (octahedral packing)
//! are read as `mcpl.c` reads them; files are always written as MCPL-3.
//!
//! A `.gz` file (`mcpl_closeandgzip_outfile`, and OpenMC's `source.10.mcpl.gz`)
//! is detected by its gzip magic and inflated with `miniz_oxide`, the DEFLATE
//! codec already in this crate's tree.
//!
//! # Units
//!
//! MCPL stores kinetic energy in **MeV** and time in **ms**; OpenMC (and this
//! crate) uses eV and s. The conversions are upstream's (`site.E = ekin * 1e6`,
//! `site.time = time * 1e-3`) and live only in the OpenMC-facing functions;
//! [`McplParticle`] holds MCPL's own units.
//!
//! # Verified
//!
//! Upstream's fixture `tests/regression_tests/surface_source/surface_source_true.mcpl`
//! (written by `libmcpl` from OpenMC 0.15.3-dev149) decodes and re-encodes
//! **byte for byte**, and files written here are read back by the reference
//! Python reader (`mcpl` 2.2.8): see
//! `verification_and_validation/collision_track_mcpl/README.md`.

use crate::geometry::position::{Direction, Position};
use njoy_outram_park_fork::hdf5::collision_track::CollisionTrackSite;

/// What [`write_mcpl_source_point`] and [`write_mcpl_collision_track`] put in
/// the `srcname` field. Upstream writes `"OpenMC <version>"`; this port is not
/// OpenMC (`TRADEMARKS.md`) and says so.
pub const SOURCE_NAME: &str = concat!("outram-mc-libs ", env!("CARGO_PKG_VERSION"));

/// `MCPLIMP_MAX_PARTICLE_SIZE` (`mcpl.c`).
const MAX_PARTICLE_SIZE: usize = 96;
/// `MCPL_STATSUMKEY_MAXLENGTH`.
const STATSUM_KEY_MAX: usize = 64;
/// `MCPL_STATSUMVAL_LENGTH`.
const STATSUM_VAL_LEN: usize = 24;
/// `MCPL_STATSUMINI`.
const STATSUM_PREFIX: &str = "stat:sum:";
/// The `stat:sum` key OpenMC writes (`write_mcpl_source_point`).
pub const OPENMC_NP1_KEY: &str = "openmc_np1";

/// An MCPL read or write error, with `mcpl.c`'s message where it has one.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("MCPL: {0}")]
pub struct McplError(pub String);

fn err<T>(msg: impl Into<String>) -> Result<T, McplError> {
    Err(McplError(msg.into()))
}

/// One particle, `mcpl_particle_t`, in MCPL's own units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct McplParticle {
    /// Kinetic energy \[MeV\].
    pub ekin: f64,
    /// Polarisation vector (zero unless the file carries polarisation).
    pub polarisation: [f64; 3],
    /// Position \[cm\].
    pub position: [f64; 3],
    /// Unit direction.
    pub direction: [f64; 3],
    /// Time \[ms\].
    pub time: f64,
    /// Statistical weight.
    pub weight: f64,
    /// PDG code (2112 for a neutron).
    pub pdgcode: i32,
    /// User flags (zero unless the file carries them).
    pub userflags: u32,
}

/// The header of an MCPL file.
#[derive(Debug, Clone, PartialEq)]
pub struct McplHeader {
    /// Format version read (2 or 3). Files are always **written** as 3.
    pub format_version: u32,
    /// `srcname`: the program that wrote the file.
    pub source_name: String,
    /// Comments, `stat:sum:` entries included.
    pub comments: Vec<String>,
    /// Binary blobs, `(key, data)`, in file order.
    pub blobs: Vec<(String, Vec<u8>)>,
    /// Particles carry user flags.
    pub userflags: bool,
    /// Particles carry a polarisation vector.
    pub polarisation: bool,
    /// Floating-point fields are `f32` (`mcpl.c`'s default) rather than `f64`.
    pub single_precision: bool,
    /// Non-zero: every particle has this PDG code and none is stored.
    pub universal_pdgcode: i32,
    /// Non-zero: every particle has this weight and none is stored.
    pub universal_weight: f64,
}

impl Default for McplHeader {
    /// `mcpl_create_outfile`'s defaults: single precision, no options,
    /// `srcname = "unknown"`.
    fn default() -> Self {
        Self {
            format_version: 3,
            source_name: "unknown".into(),
            comments: Vec::new(),
            blobs: Vec::new(),
            userflags: false,
            polarisation: false,
            single_precision: true,
            universal_pdgcode: 0,
            universal_weight: 0.0,
        }
    }
}

impl McplHeader {
    /// Bytes per particle, `mcpl_recalc_psize`.
    pub fn particle_size(&self) -> usize {
        let fp = if self.single_precision { 4 } else { 8 };
        let mut n = 7 * fp;
        if self.polarisation {
            n += 3 * fp;
        }
        if self.universal_pdgcode == 0 {
            n += 4;
        }
        if self.universal_weight == 0.0 {
            n += fp;
        }
        if self.userflags {
            n += 4;
        }
        n
    }

    /// `mcpl_hdr_stat_sum`: the value of `stat:sum:<key>`, `None` when absent.
    /// `-1` means "not available" (a file not closed properly).
    pub fn stat_sum(&self, key: &str) -> Option<f64> {
        self.comments.iter().find_map(|c| match parse_stat_sum(c) {
            Some(Ok((k, v))) if k == key => Some(v),
            _ => None,
        })
    }

    /// `mcpl_hdr_add_stat_sum` before the header is written: replace the
    /// value of an existing key in place, or add a new comment.
    pub fn set_stat_sum(&mut self, key: &str, value: f64) -> Result<(), McplError> {
        let comment = encode_stat_sum(key, value)?;
        for c in self.comments.iter_mut() {
            if let Some(Ok((k, _))) = parse_stat_sum(c) {
                if k == key {
                    *c = comment;
                    return Ok(());
                }
            }
        }
        self.comments.push(comment);
        Ok(())
    }
}

/// A whole MCPL file in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct McplFile {
    /// The header.
    pub header: McplHeader,
    /// The particles, in file order.
    pub particles: Vec<McplParticle>,
    /// What `mcpl.c` would have printed while reading (an unclosed file
    /// recovered, unknown `stat:` syntax). Empty for a clean file; not written.
    pub warnings: Vec<String>,
}

// ───────────────────────── direction packing ─────────────────────────

/// `mcpl_unitvect_pack_adaptproj`: three numbers, the third a sign (±1).
pub fn pack_adaptproj(d: [f64; 3]) -> [f64; 3] {
    let (ax, ay) = (d[0].abs(), d[1].abs());
    let mut out = if d[2].abs() < ax.max(ay) {
        let invz = if d[2] != 0.0 { 1.0 / d[2] } else { f64::INFINITY };
        if ax >= ay {
            [invz, d[1], d[0]]
        } else {
            [d[0], invz, d[1]]
        }
    } else {
        d
    };
    out[2] = 1.0_f64.copysign(out[2]);
    out
}

/// `mcpl_unitvect_unpack_adaptproj`.
pub fn unpack_adaptproj(p: [f64; 3]) -> [f64; 3] {
    if p[0].abs() > 1.0 {
        let z = 1.0 / p[0];
        [p[2] * (1.0 - (p[1] * p[1] + z * z)).max(0.0).sqrt(), p[1], z]
    } else if p[1].abs() > 1.0 {
        let z = 1.0 / p[1];
        [p[0], p[2] * (1.0 - (p[0] * p[0] + z * z)).max(0.0).sqrt(), z]
    } else {
        [p[0], p[1], p[2] * (1.0 - (p[0] * p[0] + p[1] * p[1])).max(0.0).sqrt()]
    }
}

/// `mcpl_unitvect_unpack_oct`: MCPL-2's octahedral packing, read only.
fn unpack_oct(p: [f64; 2]) -> [f64; 3] {
    let mut o = [p[0], p[1], 1.0 - p[0].abs() - p[1].abs()];
    if o[2] < 0.0 {
        o[0] = (1.0 - p[1].abs()) * if p[0] >= 0.0 { 1.0 } else { -1.0 };
        o[1] = (1.0 - p[0].abs()) * if p[1] >= 0.0 { 1.0 } else { -1.0 };
    }
    let n = 1.0 / (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
    [o[0] * n, o[1] * n, o[2] * n]
}

// ───────────────────────── stat:sum and %g ─────────────────────────

/// C's `printf("%.<p>g", v)`, which `mcpl.c` (`%24.15g`) and C++'s default
/// `ostream << double` (precision 6) both use.
pub fn format_c_g(v: f64, p: usize) -> String {
    let p = p.max(1);
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let e = format!("{:.*e}", p - 1, v);
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let x: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };
    if x < -4 || x >= p as i32 {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", strip(mant), sign, x.abs())
    } else {
        strip(&format!("{:.*}", (p as i32 - 1 - x) as usize, v))
    }
}

/// `mcpl_check_isidentifier`: `[a-zA-Z][a-zA-Z0-9_]*`.
fn is_identifier(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(a) if a.is_ascii_alphabetic())
        && c.all(|a| a.is_ascii_alphanumeric() || a == '_')
}

/// `mcpl_internal_encodestatsum`: `"stat:sum:<key>:<value, 24 wide>"`, the
/// value at `%.15g` unless that does not round-trip, then `%.17g`.
pub fn encode_stat_sum(key: &str, value: f64) -> Result<String, McplError> {
    if value.is_nan() || value.is_infinite() {
        return err("Invalid value for \"stat:sum:...\" (NaN or INF)");
    }
    if !(value >= 0.0 || value == -1.0) {
        return err(format!(
            "Invalid value for \"stat:sum:...\". Value is negative but is not -1.0 (it is {value})."
        ));
    }
    if key.is_empty() || key.len() > STATSUM_KEY_MAX || !is_identifier(key) {
        return err(format!("Invalid stat:sum: key \"{key}\""));
    }
    let val = if value == 0.0 {
        format!("{:>w$}", "0", w = STATSUM_VAL_LEN)
    } else {
        let v15 = format!("{:>w$}", format_c_g(value, 15), w = STATSUM_VAL_LEN);
        if v15.trim().parse::<f64>().ok() == Some(value) {
            v15
        } else {
            format!("{:>w$}", format_c_g(value, 17), w = STATSUM_VAL_LEN)
        }
    };
    if val.len() != STATSUM_VAL_LEN {
        return err("stat:sum: value encoding length error");
    }
    Ok(format!("{STATSUM_PREFIX}{key}:{val}"))
}

/// `mcpl_internal_statsumparse`: `None` when the comment is not a
/// `stat:sum:` entry, `Some(Err)` when it is one with bad syntax.
pub fn parse_stat_sum(comment: &str) -> Option<Result<(String, f64), McplError>> {
    let rest = comment.strip_prefix(STATSUM_PREFIX)?;
    let bad = |m: &str| Some(err(format!("invalid stat:sum: comment \"{comment}\": {m}")));
    let Some((key, val)) = rest.split_once(':') else {
        return bad("did not find colon separating key and value");
    };
    if key.is_empty() || key.len() > STATSUM_KEY_MAX || !is_identifier(key) {
        return bad("key does not adhere to naming [a-zA-Z][a-zA-Z0-9_]*");
    }
    if val.len() != STATSUM_VAL_LEN {
        return bad("value field is not exactly 24 characters wide");
    }
    let v = val.trim_matches(' ');
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit() || ".-+eE".contains(c)) {
        return bad("value field missing or holds forbidden characters");
    }
    match v.parse::<f64>() {
        Ok(x) if x.is_finite() && (x >= 0.0 || x == -1.0) => Some(Ok((key.to_string(), x))),
        _ => bad("value field must hold a non-negative value or -1"),
    }
}

// ───────────────────────── gzip ─────────────────────────

/// The CRC-32 of the gzip trailer (IEEE 802.3, reflected 0xEDB88320).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Whether `bytes` start with the gzip magic.
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

/// Inflate a single-member gzip stream (RFC 1952), checking CRC-32 and size.
pub fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, McplError> {
    if bytes.len() < 18 || !is_gzip(bytes) || bytes[2] != 8 {
        return err("not a gzip (deflate) stream");
    }
    let flg = bytes[3];
    let mut i = 10;
    let need = |i: usize, n: usize| -> Result<(), McplError> {
        if i + n > bytes.len() { err("truncated gzip header") } else { Ok(()) }
    };
    if flg & 0x04 != 0 {
        need(i, 2)?;
        let xlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2 + xlen;
    }
    for bit in [0x08_u8, 0x10] {
        if flg & bit != 0 {
            while i < bytes.len() && bytes[i] != 0 {
                i += 1;
            }
            i += 1;
        }
    }
    if flg & 0x02 != 0 {
        i += 2;
    }
    need(i, 8)?;
    let body = &bytes[i..bytes.len() - 8];
    let out = miniz_oxide::inflate::decompress_to_vec(body)
        .map_err(|e| McplError(format!("gzip inflate failed: {e:?}")))?;
    let t = &bytes[bytes.len() - 8..];
    let crc = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
    let isize = u32::from_le_bytes([t[4], t[5], t[6], t[7]]);
    if crc32(&out) != crc || out.len() as u32 != isize {
        return err("gzip CRC-32 or size mismatch (corrupt or multi-member file)");
    }
    Ok(out)
}

/// Wrap `data` in a gzip member (deflate level 6, no name, OS = unknown):
/// what `mcpl_closeandgzip_outfile` produces, modulo the compressor.
pub fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(data, 6));
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

// ───────────────────────── read ─────────────────────────

struct Cursor<'b> {
    b: &'b [u8],
    i: usize,
    le: bool,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize, what: &str) -> Result<&[u8], McplError> {
        if self.i + n > self.b.len() {
            return err(format!("Errors encountered while attempting to read {what}"));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self, what: &str) -> Result<[u8; N], McplError> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N, what)?);
        if !self.le {
            a.reverse();
        }
        Ok(a)
    }
    fn u32(&mut self, what: &str) -> Result<u32, McplError> {
        Ok(u32::from_le_bytes(self.arr(what)?))
    }
    fn u64(&mut self, what: &str) -> Result<u64, McplError> {
        Ok(u64::from_le_bytes(self.arr(what)?))
    }
    fn f64(&mut self, what: &str) -> Result<f64, McplError> {
        Ok(f64::from_le_bytes(self.arr(what)?))
    }
    fn f32(&mut self, what: &str) -> Result<f64, McplError> {
        Ok(f64::from(f32::from_le_bytes(self.arr(what)?)))
    }
    fn fp(&mut self, single: bool, what: &str) -> Result<f64, McplError> {
        if single { self.f32(what) } else { self.f64(what) }
    }
    /// `mcpl_read_buffer`: u32 length + bytes.
    fn buffer(&mut self, what: &str) -> Result<Vec<u8>, McplError> {
        let n = self.u32(what)? as usize;
        Ok(self.take(n, what)?.to_vec())
    }
    fn string(&mut self, what: &str) -> Result<String, McplError> {
        Ok(String::from_utf8_lossy(&self.buffer(what)?).into_owned())
    }
}

/// Decode an MCPL file (`mcpl_open_file` + `mcpl_read` for every particle),
/// plain or gzipped. Either byte order is read.
pub fn decode_mcpl(bytes: &[u8]) -> Result<McplFile, McplError> {
    let gz = is_gzip(bytes);
    let inflated;
    let bytes = if gz {
        inflated = gunzip(bytes)?;
        &inflated[..]
    } else {
        bytes
    };
    if bytes.len() >= 4 && &bytes[..4] != b"MCPL" {
        return err("File is not an MCPL file!");
    }
    if bytes.len() < 8 {
        return err("Error while reading first bytes of file!");
    }
    let digit = |c: u8| u32::from(c.wrapping_sub(b'0'));
    let version = digit(bytes[4]) * 100 + digit(bytes[5]) * 10 + digit(bytes[6]);
    if version != 2 && version != 3 {
        return err("File is in an unsupported MCPL version!");
    }
    let le = match bytes[7] {
        b'L' => true,
        b'B' => false,
        _ => return err("Unexpected value in endianness field!"),
    };
    let mut c = Cursor { b: bytes, i: 8, le };
    let hdr = "header";
    let mut nparticles = c.u64(hdr)?;
    let mut a = [0u32; 8];
    for x in a.iter_mut() {
        *x = c.u32(hdr)?;
    }
    let mut h = McplHeader {
        format_version: version,
        source_name: String::new(),
        comments: Vec::new(),
        blobs: Vec::new(),
        userflags: a[2] != 0,
        polarisation: a[3] != 0,
        single_precision: a[4] != 0,
        universal_pdgcode: a[5] as i32,
        universal_weight: 0.0,
    };
    let psize = a[6] as usize;
    if psize > MAX_PARTICLE_SIZE {
        return err("unexpected particle size");
    }
    if a[7] != 0 {
        h.universal_weight = c.f64(hdr)?;
    }
    if psize != h.particle_size() {
        return err(format!(
            "particle size {psize} in the header does not match the options ({})",
            h.particle_size()
        ));
    }
    h.source_name = c.string(hdr)?;
    let mut warnings = Vec::new();
    let mut unknown_stat = false;
    let mut keys: Vec<String> = Vec::new();
    for _ in 0..a[0] {
        let s = c.string(hdr)?;
        match parse_stat_sum(&s) {
            Some(Ok((k, _))) => {
                if keys.contains(&k) {
                    return err(format!(
                        "Duplicate stat:sum: key. The key \"{k}\" appears more than once in the file."
                    ));
                }
                keys.push(k);
            }
            Some(Err(e)) => return Err(e),
            None if s.starts_with("stat:") => unknown_stat = true,
            None => {}
        }
        h.comments.push(s);
    }
    if unknown_stat {
        warnings.push(
            "MCPL WARNING: Opened file with unknown \"stat:...\" syntax in comments.".into(),
        );
    }
    let blob_keys: Vec<String> = (0..a[1]).map(|_| c.string(hdr)).collect::<Result<_, _>>()?;
    for k in blob_keys {
        let data = c.buffer(hdr)?;
        h.blobs.push((k, data));
    }
    let first = c.i;
    // An unclosed file (nparticles still 0) is recovered from its size, as
    // `mcpl_actual_open_file` does, its stat:sum entries marked -1.
    if nparticles == 0 && bytes.len() > first {
        if gz {
            return err(
                "Input file appears to not have been closed properly and data recovery is \
                 disabled for gzipped files.",
            );
        }
        let np = ((bytes.len() - first) / psize.max(1)) as u64;
        if np > 0 {
            warnings.push(format!(
                "MCPL WARNING: Input file appears to not have been closed properly. \
                 Recovered {np} particles."
            ));
            nparticles = np;
            for cm in h.comments.iter_mut() {
                if let Some(Ok((k, v))) = parse_stat_sum(cm) {
                    if v != -1.0 {
                        *cm = encode_stat_sum(&k, -1.0)?;
                        warnings.push(format!(
                            "MCPL WARNING: Marking stat:sum:{k} entry as not available (-1) \
                             since file not closed properly."
                        ));
                    }
                }
            }
        }
    }
    let mut particles = Vec::with_capacity(nparticles.min(1 << 24) as usize);
    let pd = "particle data";
    let sp = h.single_precision;
    for _ in 0..nparticles {
        let mut p = McplParticle {
            ekin: 0.0,
            polarisation: [0.0; 3],
            position: [0.0; 3],
            direction: [0.0; 3],
            time: 0.0,
            weight: h.universal_weight,
            pdgcode: h.universal_pdgcode,
            userflags: 0,
        };
        if h.polarisation {
            for k in 0..3 {
                p.polarisation[k] = c.fp(sp, pd)?;
            }
        }
        for k in 0..3 {
            p.position[k] = c.fp(sp, pd)?;
        }
        let mut pk = [0.0; 3];
        for x in pk.iter_mut() {
            *x = c.fp(sp, pd)?;
        }
        p.time = c.fp(sp, pd)?;
        if p.weight == 0.0 {
            p.weight = c.fp(sp, pd)?;
        }
        if h.universal_pdgcode == 0 {
            p.pdgcode = c.u32(pd)? as i32;
        }
        if h.userflags {
            p.userflags = c.u32(pd)?;
        }
        if version >= 3 {
            p.ekin = pk[2].abs();
            pk[2] = 1.0_f64.copysign(pk[2]);
            p.direction = unpack_adaptproj(pk);
        } else {
            p.direction = unpack_oct([pk[0], pk[1]]);
            p.ekin = pk[2];
            if pk[2].is_sign_negative() {
                p.ekin = -p.ekin;
                p.direction[2] = 0.0;
            }
        }
        particles.push(p);
    }
    Ok(McplFile { header: h, particles, warnings })
}

// ───────────────────────── write ─────────────────────────

fn put_buffer(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

/// Encode an MCPL-3 little-endian file: `mcpl_write_header`, then
/// `mcpl_internal_serialise_particle_to_buffer` for every particle and the
/// particle count `mcpl_close_outfile` writes back. `mcpl.c`'s checks are
/// kept: a non-unit direction (`|d·d − 1| > 1e-5`), a negative energy, a
/// duplicate blob key, a malformed or duplicate `stat:sum:` comment and any
/// other comment starting `stat:` are refused.
pub fn encode_mcpl(file: &McplFile) -> Result<Vec<u8>, McplError> {
    let h = &file.header;
    let mut keys: Vec<&str> = Vec::new();
    for c in &h.comments {
        match parse_stat_sum(c) {
            Some(Ok((k, _))) => {
                if keys.iter().any(|x| *x == k) {
                    return err(format!("Duplicate stat:sum: key \"{k}\""));
                }
                keys.push(&c[STATSUM_PREFIX.len()..STATSUM_PREFIX.len() + k.len()]);
            }
            Some(Err(e)) => return Err(e),
            None if c.starts_with("stat:") => {
                return err(
                    "Refusing to create file with comments starting with \"stat:\" unless \
                     starting with \"stat:sum:\", as such syntax is reserved for future usage.",
                )
            }
            None => {}
        }
    }
    for (i, (k, _)) in h.blobs.iter().enumerate() {
        if h.blobs[..i].iter().any(|(j, _)| j == k) {
            return err("mcpl_hdr_add_data got duplicate key");
        }
    }
    let psize = h.particle_size();
    let mut out = Vec::with_capacity(64 + file.particles.len() * psize);
    out.extend_from_slice(b"MCPL003L");
    out.extend_from_slice(&(file.particles.len() as u64).to_le_bytes());
    for v in [
        h.comments.len() as u32,
        h.blobs.len() as u32,
        u32::from(h.userflags),
        u32::from(h.polarisation),
        u32::from(h.single_precision),
        h.universal_pdgcode as u32,
        psize as u32,
        u32::from(h.universal_weight != 0.0),
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    if h.universal_weight != 0.0 {
        out.extend_from_slice(&h.universal_weight.to_le_bytes());
    }
    put_buffer(&mut out, h.source_name.as_bytes());
    for c in &h.comments {
        put_buffer(&mut out, c.as_bytes());
    }
    for (k, _) in &h.blobs {
        put_buffer(&mut out, k.as_bytes());
    }
    for (_, d) in &h.blobs {
        put_buffer(&mut out, d);
    }
    let put = |out: &mut Vec<u8>, v: f64| {
        if h.single_precision {
            out.extend_from_slice(&(v as f32).to_le_bytes());
        } else {
            out.extend_from_slice(&v.to_le_bytes());
        }
    };
    for p in &file.particles {
        let d = p.direction;
        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - 1.0).abs() > 1.0e-5 {
            return err("attempting to add particle with non-unit direction vector");
        }
        if p.ekin < 0.0 {
            return err("attempting to add particle with negative kinetic energy");
        }
        let mut pk = pack_adaptproj(d);
        pk[2] = p.ekin.copysign(pk[2]);
        if h.polarisation {
            for v in p.polarisation {
                put(&mut out, v);
            }
        }
        for v in p.position.iter().chain(&pk) {
            put(&mut out, *v);
        }
        put(&mut out, p.time);
        if h.universal_weight == 0.0 {
            put(&mut out, p.weight);
        }
        if h.universal_pdgcode == 0 {
            out.extend_from_slice(&p.pdgcode.to_le_bytes());
        }
        if h.userflags {
            out.extend_from_slice(&p.userflags.to_le_bytes());
        }
    }
    Ok(out)
}

/// `mcpl_create_outfile`'s name rule: `.mcpl` is appended unless the name
/// already ends with it.
pub fn mcpl_outfile_name(name: &str) -> Result<String, McplError> {
    if name.is_empty() {
        return err("mcpl_create_outfile called with empty string.");
    }
    if name == ".mcpl" {
        return err("mcpl_create_outfile called with string with no basename part (\".mcpl\").");
    }
    Ok(if name.ends_with(".mcpl") { name.to_string() } else { format!("{name}.mcpl") })
}

/// Read an MCPL file from disk (plain or `.gz`). Run-boundary I/O only.
pub fn read_mcpl_file(path: &str) -> Result<McplFile, McplError> {
    let b = std::fs::read(path).map_err(|e| McplError(format!("Unable to open file! {path}: {e}")))?;
    decode_mcpl(&b)
}

/// Write an MCPL file to disk under [`mcpl_outfile_name`]; returns the path.
/// Run-boundary I/O only.
pub fn write_mcpl_file(name: &str, file: &McplFile) -> Result<String, McplError> {
    let path = mcpl_outfile_name(name)?;
    let bytes = encode_mcpl(file)?;
    std::fs::write(&path, bytes).map_err(|e| McplError(format!("Unable to open output file! {e}")))?;
    Ok(path)
}

// ───────────────────────── OpenMC's uses ─────────────────────────

/// OpenMC's `SourceSite` as an MCPL file carries it: everything a source
/// file can set, in OpenMC's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleSite {
    /// Position \[cm\].
    pub r: Position,
    /// Direction.
    pub u: Direction,
    /// Energy \[eV\].
    pub e: f64,
    /// Time \[s\].
    pub time: f64,
    /// Weight.
    pub wgt: f64,
    /// PDG code.
    pub particle: i32,
}

/// `mcpl_particle_to_site`: MeV to eV, ms to s.
pub fn mcpl_particle_to_site(p: &McplParticle) -> ParticleSite {
    ParticleSite {
        r: Position::new(p.position[0], p.position[1], p.position[2]),
        u: Direction::new(p.direction[0], p.direction[1], p.direction[2]),
        e: p.ekin * 1e6,
        time: p.time * 1e-3,
        wgt: p.weight,
        particle: p.pdgcode,
    }
}

/// The reverse, as `write_mcpl_source_bank_internal` fills
/// `mcpl_particle_repr_t`: eV to MeV, s to ms, no polarisation or flags.
pub fn site_to_mcpl_particle(s: &ParticleSite) -> McplParticle {
    McplParticle {
        ekin: s.e * 1e-6,
        polarisation: [0.0; 3],
        position: [s.r.x, s.r.y, s.r.z],
        direction: [s.u.u, s.u.v, s.u.w],
        time: s.time * 1e3,
        weight: s.wgt,
        pdgcode: s.particle,
        userflags: 0,
    }
}

/// `mcpl_source_sites`: every particle of an MCPL file as a source site.
/// Upstream's fatal errors are errors here: a file that cannot be opened, and
/// a file with no particles ("is empty or contains no particle data").
pub fn mcpl_source_sites(bytes: &[u8]) -> Result<Vec<ParticleSite>, McplError> {
    let f = decode_mcpl(bytes)?;
    if f.particles.is_empty() {
        return err("MCPL file is empty or contains no particle data.");
    }
    Ok(f.particles.iter().map(mcpl_particle_to_site).collect())
}

/// `write_mcpl_source_point`: a source (or surface-source) bank as an MCPL
/// file, with upstream's `stat:sum:openmc_np1` entry. `n_source_particles`
/// is what upstream computes as `(n_batches - n_inactive) * gen_per_batch *
/// n_particles`, the run's source particles rather than the number in the
/// file; `None` leaves upstream's crash-safety value `-1`.
pub fn write_mcpl_source_point(
    sites: &[ParticleSite],
    n_source_particles: Option<i64>,
) -> Result<McplFile, McplError> {
    let mut header = McplHeader { source_name: SOURCE_NAME.into(), ..McplHeader::default() };
    header.set_stat_sum(OPENMC_NP1_KEY, -1.0)?;
    if let Some(n) = n_source_particles {
        header.set_stat_sum(OPENMC_NP1_KEY, n as f64)?;
    }
    Ok(McplFile {
        header,
        particles: sites.iter().map(site_to_mcpl_particle).collect(),
        warnings: Vec::new(),
    })
}

/// The text blob `write_mcpl_collision_track_internal` stores for one site,
/// `" dE : <dE> ; event_mt : <mt> ; ... ; progeny_id : <n>"`, with `dE` at
/// C++'s default stream precision (6 significant figures, as upstream).
pub fn collision_track_blob(s: &CollisionTrackSite) -> String {
    format!(
        " dE : {} ; event_mt : {} ; delayed_group : {} ; cell_id : {} ; nuclide_id : {} ; \
         material_id : {} ; universe_id : {} ; n_collision : {} ; parent_id : {} ; \
         progeny_id : {}",
        format_c_g(s.de, 6),
        s.event_mt,
        s.delayed_group,
        s.cell_id,
        s.nuclide_id,
        s.material_id,
        s.universe_id,
        s.n_collision,
        s.parent_id,
        s.progeny_id
    )
}

/// `write_mcpl_collision_track`: one particle per collision (E in MeV, time
/// in ms) and one blob per collision, keyed `blob_<i>`, carrying the fields
/// MCPL has no column for ([`collision_track_blob`]).
pub fn write_mcpl_collision_track(sites: &[CollisionTrackSite]) -> McplFile {
    let header = McplHeader {
        source_name: SOURCE_NAME.into(),
        blobs: sites
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("blob_{i}"), collision_track_blob(s).into_bytes()))
            .collect(),
        ..McplHeader::default()
    };
    let particles = sites
        .iter()
        .map(|s| McplParticle {
            ekin: s.e * 1e-6,
            polarisation: [0.0; 3],
            position: s.r,
            direction: s.u,
            time: s.time * 1e3,
            weight: s.wgt,
            pdgcode: s.particle,
            userflags: 0,
        })
        .collect();
    McplFile { header, particles, warnings: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_g_formatting_matches_printf() {
        assert_eq!(format_c_g(10000.0, 15), "10000");
        assert_eq!(format_c_g(-1.0, 15), "-1");
        assert_eq!(format_c_g(0.1, 15), "0.1");
        assert_eq!(format_c_g(1.0e-5, 6), "1e-05");
        assert_eq!(format_c_g(123456789.0, 6), "1.23457e+08");
        assert_eq!(format_c_g(1079.78123, 6), "1079.78");
        assert_eq!(format_c_g(0.0001234, 6), "0.0001234");
        assert_eq!(format_c_g(100000.0, 6), "100000");
        assert_eq!(format_c_g(1000000.0, 6), "1e+06");
    }

    #[test]
    fn stat_sum_encodes_as_mcpl_does_and_parses_back() {
        let c = encode_stat_sum("openmc_np1", 10000.0).unwrap();
        assert_eq!(c, "stat:sum:openmc_np1:                   10000");
        assert_eq!(parse_stat_sum(&c), Some(Ok(("openmc_np1".into(), 10000.0))));
        let c = encode_stat_sum("k", 0.1 + 0.2).unwrap();
        assert_eq!(parse_stat_sum(&c).unwrap().unwrap().1, 0.1 + 0.2, "{c}");
        assert!(encode_stat_sum("k", -2.0).is_err());
        assert!(encode_stat_sum("1k", 1.0).is_err());
        assert!(parse_stat_sum("plain comment").is_none());
        assert!(parse_stat_sum("stat:sum:k:1").unwrap().is_err());
    }

    #[test]
    fn adaptive_projection_round_trips_every_branch() {
        for d in [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.6, 0.0, 0.8],
            [0.8, -0.6, 0.0],
            [-0.48, 0.8, 0.36],
        ] {
            let b = unpack_adaptproj(pack_adaptproj(d));
            for k in 0..3 {
                assert!((b[k] - d[k]).abs() < 1e-15, "{d:?} -> {b:?}");
            }
        }
        let o = unpack_oct([0.25, -0.25]);
        assert!(((o[0] * o[0] + o[1] * o[1] + o[2] * o[2]) - 1.0).abs() < 1e-15);
    }

    fn particle(i: usize) -> McplParticle {
        let a = i as f64 * 0.37;
        let (s, c) = (a.sin(), a.cos());
        McplParticle {
            ekin: 1.0e-8 * (i + 1) as f64,
            polarisation: [0.0; 3],
            position: [i as f64, -1.5, 2.25],
            direction: [s * 0.6, c * 0.6, if i % 2 == 0 { 0.8 } else { -0.8 }],
            time: 0.5,
            weight: 1.0 + i as f64,
            pdgcode: 2112,
            userflags: 0,
        }
    }

    #[test]
    fn every_option_round_trips_in_double_precision_and_gzip() {
        let mut header = McplHeader {
            single_precision: false,
            polarisation: true,
            userflags: true,
            source_name: "test".into(),
            blobs: vec![("a".into(), vec![1, 2, 3])],
            ..McplHeader::default()
        };
        header.set_stat_sum("n", 7.0).unwrap();
        let mut ps: Vec<McplParticle> = (0..9).map(particle).collect();
        ps[3].polarisation = [0.1, 0.2, 0.3];
        ps[4].userflags = 0xdead_beef;
        let f = McplFile { header, particles: ps, warnings: Vec::new() };
        let b = encode_mcpl(&f).unwrap();
        let g = decode_mcpl(&b).unwrap();
        assert_eq!(g.header, f.header);
        for (x, y) in g.particles.iter().zip(&f.particles) {
            assert_eq!((x.ekin, x.position, x.weight, x.userflags), (y.ekin, y.position, y.weight, y.userflags));
            for k in 0..3 {
                assert!((x.direction[k] - y.direction[k]).abs() < 1e-15);
            }
        }
        assert_eq!(encode_mcpl(&g).unwrap(), b, "decode then encode is the identity");
        assert_eq!(decode_mcpl(&gzip(&b)).unwrap().particles, g.particles);

        // universal weight and pdg
        let h = McplHeader { universal_weight: 2.5, universal_pdgcode: 22, ..McplHeader::default() };
        let f = McplFile { header: h, particles: vec![particle(0)], warnings: vec![] };
        let g = decode_mcpl(&encode_mcpl(&f).unwrap()).unwrap();
        assert_eq!((g.particles[0].weight, g.particles[0].pdgcode), (2.5, 22));
        assert_eq!(g.header.particle_size(), 28, "7 f32: position, packed direction+ekin, time");
    }

    #[test]
    fn mcpl_c_refusals_are_kept() {
        let mut f = McplFile { header: McplHeader::default(), particles: vec![particle(1)], warnings: vec![] };
        f.particles[0].direction = [1.0, 1.0, 0.0];
        assert!(encode_mcpl(&f).is_err());
        f.particles[0] = particle(1);
        f.particles[0].ekin = -1.0;
        assert!(encode_mcpl(&f).is_err());
        f.particles[0] = particle(1);
        f.header.comments.push("stat:foo".into());
        assert!(encode_mcpl(&f).is_err());
        assert!(decode_mcpl(b"MCPX0000").is_err());
        assert!(decode_mcpl(b"MCPL004L").is_err());
        assert!(mcpl_outfile_name(".mcpl").is_err());
        assert_eq!(mcpl_outfile_name("a/b").unwrap(), "a/b.mcpl");
        assert_eq!(mcpl_outfile_name("a.mcpl").unwrap(), "a.mcpl");
    }

    /// An unclosed file (particle count left at 0) is recovered from its size
    /// and its `stat:sum` marked unavailable, as `mcpl_actual_open_file` does.
    #[test]
    fn an_unclosed_file_is_recovered() {
        let f = write_mcpl_source_point(
            &[mcpl_particle_to_site(&particle(2)), mcpl_particle_to_site(&particle(3))],
            Some(1000),
        )
        .unwrap();
        let mut b = encode_mcpl(&f).unwrap();
        b[8..16].copy_from_slice(&0u64.to_le_bytes());
        let g = decode_mcpl(&b).unwrap();
        assert_eq!(g.particles.len(), 2);
        assert_eq!(g.header.stat_sum(OPENMC_NP1_KEY), Some(-1.0));
        assert_eq!(g.warnings.len(), 2);
        assert!(decode_mcpl(&gzip(&b)).is_err(), "no recovery for gzip");
    }

    /// OpenMC's unit conversions and the empty-file refusal.
    #[test]
    fn openmc_source_sites_convert_units() {
        let f = write_mcpl_source_point(&[mcpl_particle_to_site(&particle(4))], Some(10)).unwrap();
        assert_eq!(f.header.stat_sum(OPENMC_NP1_KEY), Some(10.0));
        let s = mcpl_source_sites(&encode_mcpl(&f).unwrap()).unwrap();
        assert!((s[0].e - 5.0e-2).abs() < 1e-8 && (s[0].time - 5.0e-4).abs() < 1e-10);
        let empty = write_mcpl_source_point(&[], None).unwrap();
        assert_eq!(empty.header.stat_sum(OPENMC_NP1_KEY), Some(-1.0));
        assert!(mcpl_source_sites(&encode_mcpl(&empty).unwrap()).is_err());
    }
}
