//! The pure-Rust embedded formats: **WMPB v1** (one nuclide —
//! [`WindowedMultipole::to_blob`] / [`WindowedMultipole::from_blob`]) and the
//! **WMPL v1** container ([`WmpLibrary`]) that packs the CORE nuclide set.
//!
//! Split out of `wmp.rs` (see the module doc there for provenance and status).

use super::types::{Cf64, WindowedMultipole, WmpWindow};
use crate::NjoyError;
use std::sync::OnceLock;

impl WindowedMultipole {
    /// Serialize this nuclide into the compact **WMPB v1** embedded blob — the
    /// exact inverse of [`Self::from_blob`]. This is the offline *bake* step: a
    /// maintainer loads MIT `WMP_Library` HDF5 with [`Self::load_h5`] and writes
    /// the result here; the shipped crate then `include_bytes!`s the blob and
    /// decodes it with [`Self::from_blob`], so **no HDF5 dependency lives in the
    /// runtime build**.
    ///
    /// # Format — WMPB v1 (little-endian)
    /// A plaintext header + index, then a single **deflate** stream of the
    /// pole / residue / curve-fit doubles:
    ///
    /// | Bytes | Field |
    /// |---|---|
    /// | `0..4` | magic `b"WMPB"` |
    /// | `4` | version (`1`) |
    /// | `5` | flags (bit 0 = fissionable) |
    /// | `6..8` | reserved (`0`) |
    /// | `8..40` | `awr`, `e_min`, `e_max`, `inv_spacing` (4×`f64`) |
    /// | `40..56` | `fit_order`, `n_poles`, `n_windows`, `name_len` (4×`u32`) |
    /// | `56..` | name (UTF-8), then `n_windows`×(`start` u32, `end` u32, `broaden` u8) |
    /// | rest | deflate(byte-plane-shuffled doubles) |
    ///
    /// The doubles are grouped by column (all pole real parts, then all
    /// imaginary parts, then each residue/curve-fit channel) and **byte-plane
    /// shuffled** (byte 0 of every value, then byte 1, …) before deflate, so the
    /// low-entropy exponent/sign planes cluster. IEEE mantissa bits are
    /// near-incompressible, so this only buys ~1.15–1.17×, but it is free to
    /// apply. Codec is pure-Rust [`miniz_oxide`] — no C toolchain either way.
    pub fn to_blob(&self) -> Vec<u8> {
        let n_poles = self.poles.len();
        let n_windows = self.windows.len();
        let n_coeff = self.fit_order + 1;

        // Column-grouped doubles — homogeneous streams give deflate more to chew
        // on. Order here MUST match the slicing in `from_blob`.
        let mut vals: Vec<f64> = Vec::with_capacity(n_poles * 8 + n_windows * n_coeff * 3);
        vals.extend(self.poles.iter().map(|p| p.re));
        vals.extend(self.poles.iter().map(|p| p.im));
        for ch in 0..3 {
            vals.extend(self.residues.iter().map(|r| r[ch].re));
            vals.extend(self.residues.iter().map(|r| r[ch].im));
        }
        for ch in 0..3 {
            for win in &self.curvefit {
                for coeff in win {
                    vals.push(coeff[ch]);
                }
            }
        }
        let compressed = miniz_oxide::deflate::compress_to_vec(&shuffle_doubles(&vals), 10);

        let name = self.name.as_bytes();
        let mut out =
            Vec::with_capacity(WMPB_HEADER_LEN + name.len() + n_windows * 9 + compressed.len());
        out.extend_from_slice(&WMPB_MAGIC);
        out.push(WMPB_VERSION);
        out.push(if self.fissionable {
            WMPB_FLAG_FISSIONABLE
        } else {
            0
        });
        out.extend_from_slice(&[0u8, 0u8]); // reserved
        out.extend_from_slice(&self.awr.to_le_bytes());
        out.extend_from_slice(&self.e_min.to_le_bytes());
        out.extend_from_slice(&self.e_max.to_le_bytes());
        out.extend_from_slice(&self.inv_spacing.to_le_bytes());
        out.extend_from_slice(&(self.fit_order as u32).to_le_bytes());
        out.extend_from_slice(&(n_poles as u32).to_le_bytes());
        out.extend_from_slice(&(n_windows as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        // Window table. An empty window is `end < start`; its natural encoding
        // (start = 1, end = 0) already fits u32, so no sentinel is needed.
        for w in &self.windows {
            out.extend_from_slice(&(w.start as u32).to_le_bytes());
            out.extend_from_slice(&(w.end as u32).to_le_bytes());
            out.push(w.broaden_poly as u8);
        }
        out.extend_from_slice(&compressed);
        out
    }

    /// Decode a nuclide from a compact embedded **WMPB v1** blob — the in-crate,
    /// zero-dependency delivery path (no HDF5). Inverse of [`Self::to_blob`],
    /// which documents the byte format.
    ///
    /// Input is treated as untrusted: every length is bounds-checked and the
    /// deflate stream is inflated with a hard cap equal to the exact expected
    /// double-payload size (derived from the header counts), so a malformed blob
    /// fails cleanly rather than runaway-allocating (same discipline as the
    /// crate's 12 GB unit-test cap).
    ///
    /// # Errors
    /// [`NjoyError::WmpData`] on bad magic/version, a truncated header/table,
    /// invalid UTF-8 in the name, a deflate failure, or a size mismatch.
    pub fn from_blob(bytes: &[u8]) -> Result<Self, NjoyError> {
        let err = |m: String| NjoyError::WmpData(m);
        if bytes.len() < WMPB_HEADER_LEN {
            return Err(err("blob shorter than WMPB header".into()));
        }
        if bytes[0..4] != WMPB_MAGIC {
            return Err(err("bad WMPB magic".into()));
        }
        if bytes[4] != WMPB_VERSION {
            return Err(err(format!("unsupported WMPB version {}", bytes[4])));
        }
        let fissionable = bytes[5] & WMPB_FLAG_FISSIONABLE != 0;
        let rd_f64 = |o: usize| f64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
        let rd_u32 = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
        let awr = rd_f64(8);
        let e_min = rd_f64(16);
        let e_max = rd_f64(24);
        let inv_spacing = rd_f64(32);
        let fit_order = rd_u32(40);
        let n_poles = rd_u32(44);
        let n_windows = rd_u32(48);
        let name_len = rd_u32(52);

        // -- Name (UTF-8) --------------------------------------------------------
        let name_end = WMPB_HEADER_LEN
            .checked_add(name_len)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| err("truncated name".into()))?;
        let name = std::str::from_utf8(&bytes[WMPB_HEADER_LEN..name_end])
            .map_err(|e| err(format!("name utf8: {e}")))?
            .to_string();

        // -- Window table: 9 bytes each (u32 start, u32 end, u8 broaden) ---------
        let win_bytes = n_windows
            .checked_mul(9)
            .ok_or_else(|| err("window overflow".into()))?;
        let win_end = name_end
            .checked_add(win_bytes)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| err("truncated window table".into()))?;
        let mut windows = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let o = name_end + w * 9;
            let start = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
            let end = u32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()) as usize;
            windows.push(WmpWindow {
                start,
                end,
                broaden_poly: bytes[o + 8] != 0,
            });
        }

        // -- Doubles: know the exact expected count → bound the inflate ----------
        let n_coeff = fit_order
            .checked_add(1)
            .ok_or_else(|| err("fit_order overflow".into()))?;
        let n_cf = n_windows
            .checked_mul(n_coeff)
            .ok_or_else(|| err("curvefit overflow".into()))?;
        let n_vals = n_poles
            .checked_mul(8)
            .and_then(|a| n_cf.checked_mul(3).and_then(|b| a.checked_add(b)))
            .ok_or_else(|| err("doubles count overflow".into()))?;
        let expected = n_vals
            .checked_mul(8)
            .ok_or_else(|| err("doubles size overflow".into()))?;

        let planes =
            miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[win_end..], expected)
                .map_err(|e| err(format!("deflate: {e:?}")))?;
        if planes.len() != expected {
            return Err(err(format!(
                "inflated {} bytes, expected {expected}",
                planes.len()
            )));
        }
        let vals = unshuffle_doubles(&planes);

        // Slice the columns back out in the order `to_blob` wrote them.
        let mut idx = 0usize;
        let mut take = |n: usize| -> Vec<f64> {
            let s = vals[idx..idx + n].to_vec();
            idx += n;
            s
        };
        let pole_re = take(n_poles);
        let pole_im = take(n_poles);
        let res: Vec<Vec<f64>> = (0..6).map(|_| take(n_poles)).collect();
        let cf: Vec<Vec<f64>> = (0..3).map(|_| take(n_cf)).collect();

        let poles = (0..n_poles)
            .map(|i| Cf64::new(pole_re[i], pole_im[i]))
            .collect();
        let residues = (0..n_poles)
            .map(|i| {
                [
                    Cf64::new(res[0][i], res[1][i]),
                    Cf64::new(res[2][i], res[3][i]),
                    Cf64::new(res[4][i], res[5][i]),
                ]
            })
            .collect();
        let mut curvefit = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let mut coeffs = Vec::with_capacity(n_coeff);
            for c in 0..n_coeff {
                let o = w * n_coeff + c;
                coeffs.push([cf[0][o], cf[1][o], cf[2][o]]);
            }
            curvefit.push(coeffs);
        }

        Ok(WindowedMultipole {
            name,
            awr,
            e_min,
            e_max,
            fissionable,
            poles,
            residues,
            curvefit,
            windows,
            inv_spacing,
            fit_order,
        })
    }
}

/// Magic prefix identifying a WMPB (Windowed-Multipole Blob) byte stream.
const WMPB_MAGIC: [u8; 4] = *b"WMPB";
/// WMPB format version encoded/decoded by [`WindowedMultipole::to_blob`] / `from_blob`.
const WMPB_VERSION: u8 = 1;
/// Header flag bit: this nuclide carries fission residues (the third channel).
const WMPB_FLAG_FISSIONABLE: u8 = 0x01;
/// Fixed-size WMPB header before the variable-length name: magic(4) + version(1) +
/// flags(1) + reserved(2) + 4×`f64`(32) + 4×`u32`(16) = 56 bytes.
const WMPB_HEADER_LEN: usize = 56;

/// Byte-plane shuffle of an `f64` stream for the WMPB blob: emit byte 0 of every
/// value, then byte 1 of every value, and so on. Clusters the low-entropy
/// exponent/sign bytes so `deflate` finds more redundancy. Inverse of
/// [`unshuffle_doubles`].
fn shuffle_doubles(vals: &[f64]) -> Vec<u8> {
    let n = vals.len();
    let mut out = vec![0u8; n * 8];
    for (i, v) in vals.iter().enumerate() {
        let b = v.to_le_bytes();
        for k in 0..8 {
            out[k * n + i] = b[k];
        }
    }
    out
}

/// Reassemble the `f64` stream that a [`shuffle_doubles`] byte-plane layout
/// encodes. `planes.len()` must be a multiple of 8 (guaranteed by the caller,
/// which sizes the inflate to `n_vals * 8`).
fn unshuffle_doubles(planes: &[u8]) -> Vec<f64> {
    let n = planes.len() / 8;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut b = [0u8; 8];
        for k in 0..8 {
            b[k] = planes[k * n + i];
        }
        out.push(f64::from_le_bytes(b));
    }
    out
}

/// Magic prefix for a WMPL (Windowed-Multipole Library) container.
const WMPL_MAGIC: [u8; 4] = *b"WMPL";
/// WMPL container version handled by [`WmpLibrary`].
const WMPL_VERSION: u8 = 1;
/// Fixed WMPL header before the index: magic(4) + version(1) + reserved(3) +
/// `n_nuclides`(u32, 4) = 12 bytes.
const WMPL_HEADER_LEN: usize = 12;

/// One directory entry: a nuclide's name and the byte range of its WMPB blob
/// inside the owned container image.
#[derive(Debug, Clone)]
struct WmpEntry {
    name: String,
    offset: usize,
    len: usize,
}

/// An embeddable bundle of many nuclides' [`WindowedMultipole`] blobs behind a
/// single byte image — the shipping container for the in-crate CORE data set.
///
/// One [`WindowedMultipole::to_blob`] per nuclide is concatenated behind a small
/// name→range index, so the entire CORE set is a single `include_bytes!` and any
/// nuclide is decoded on demand by [`Self::get`]. The container adds **no second
/// compression pass** — each entry is an already-deflated WMPB blob, so packing
/// is just indexing + concatenation.
///
/// # Format — WMPL v1 (little-endian)
/// | Bytes | Field |
/// |---|---|
/// | `0..4` | magic `b"WMPL"` |
/// | `4` | version (`1`) |
/// | `5..8` | reserved (`0`) |
/// | `8..12` | `n_nuclides` (`u32`) |
/// | per nuclide (index) | `name_len` (u32), name (UTF-8), `blob_len` (u32) |
/// | after the index | the `n_nuclides` WMPB blobs, concatenated in index order |
///
/// Blob offsets are **implicit** — the running sum of preceding `blob_len`s — so
/// the index can never disagree with the payload.
#[derive(Debug, Clone)]
pub struct WmpLibrary {
    /// The whole WMPL container image, owned (typically copied once at startup
    /// from an `include_bytes!` static).
    bytes: Vec<u8>,
    /// name → byte range into `bytes`, in the container's stored order.
    index: Vec<WmpEntry>,
}

impl WmpLibrary {
    /// Pack a set of nuclides into a WMPL v1 container image — the offline *bake*
    /// step for the embedded CORE set. Each nuclide is serialized with
    /// [`WindowedMultipole::to_blob`]; the returned bytes are what a maintainer
    /// commits and the crate `include_bytes!`s. Input order is preserved.
    pub fn pack(nuclides: &[WindowedMultipole]) -> Vec<u8> {
        let blobs: Vec<Vec<u8>> = nuclides.iter().map(|n| n.to_blob()).collect();

        let index_len: usize = nuclides.iter().map(|n| 4 + n.name.len() + 4).sum();
        let payload_len: usize = blobs.iter().map(|b| b.len()).sum();
        let mut out = Vec::with_capacity(WMPL_HEADER_LEN + index_len + payload_len);

        out.extend_from_slice(&WMPL_MAGIC);
        out.push(WMPL_VERSION);
        out.extend_from_slice(&[0u8, 0u8, 0u8]); // reserved
        out.extend_from_slice(&(nuclides.len() as u32).to_le_bytes());
        for (n, b) in nuclides.iter().zip(&blobs) {
            let name = n.name.as_bytes();
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        }
        for b in &blobs {
            out.extend_from_slice(b);
        }
        out
    }

    /// Parse a WMPL v1 container image and take ownership of its bytes. The index
    /// is validated eagerly (magic, version, every name and byte range); the
    /// per-nuclide WMPB blobs are **not** decoded until [`Self::get`].
    ///
    /// Input is treated as untrusted: all lengths are bounds-checked so a
    /// malformed image fails cleanly rather than over-allocating.
    ///
    /// # Errors
    /// [`NjoyError::WmpData`] on bad magic/version, a truncated header/index, an
    /// out-of-range blob, or invalid UTF-8 in a name.
    pub fn from_blob(bytes: &[u8]) -> Result<Self, NjoyError> {
        let err = |m: String| NjoyError::WmpData(m);
        if bytes.len() < WMPL_HEADER_LEN {
            return Err(err("blob shorter than WMPL header".into()));
        }
        if bytes[0..4] != WMPL_MAGIC {
            return Err(err("bad WMPL magic".into()));
        }
        if bytes[4] != WMPL_VERSION {
            return Err(err(format!("unsupported WMPL version {}", bytes[4])));
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;

        let read_u32 = |o: usize| -> Result<usize, NjoyError> {
            bytes
                .get(o..o + 4)
                .map(|s| u32::from_le_bytes(s.try_into().unwrap()) as usize)
                .ok_or_else(|| err("truncated index".into()))
        };

        // Pass 1: read (name, blob_len) pairs; `cur` ends at the payload start.
        let mut cur = WMPL_HEADER_LEN;
        let mut parsed: Vec<(String, usize)> = Vec::with_capacity(n);
        for _ in 0..n {
            let name_len = read_u32(cur)?;
            cur += 4;
            let name_end = cur
                .checked_add(name_len)
                .filter(|&e| e <= bytes.len())
                .ok_or_else(|| err("truncated name in index".into()))?;
            let name = std::str::from_utf8(&bytes[cur..name_end])
                .map_err(|e| err(format!("index name utf8: {e}")))?
                .to_string();
            cur = name_end;
            let blob_len = read_u32(cur)?;
            cur += 4;
            parsed.push((name, blob_len));
        }

        // Pass 2: assign implicit offsets into the payload region.
        let mut offset = cur;
        let mut index = Vec::with_capacity(n);
        for (name, len) in parsed {
            let end = offset
                .checked_add(len)
                .filter(|&e| e <= bytes.len())
                .ok_or_else(|| err(format!("blob for {name} out of range")))?;
            index.push(WmpEntry { name, offset, len });
            offset = end;
        }

        Ok(Self {
            bytes: bytes.to_vec(),
            index,
        })
    }

    /// The embedded **CORE** nuclide set — 125 reactor-grade + LFTR nuclides
    /// (ENDF/B-VII.1 windowed multipole, MIT CRPG; see `docs/wmp-nuclide-manifest.md`),
    /// baked into the crate so every build resolves cross sections offline
    /// with no HDF5 and no downloads.
    ///
    /// The ~4.7 MB blob is parsed once and shared; repeated calls return the same
    /// cached library. Look up a nuclide with [`Self::get`] (e.g. `.get("U238")`).
    ///
    /// The blob is **always embedded** — there is no feature to disable it, so this
    /// method is available in every build. **License:** the returned data is
    /// MIT CRPG (`LICENSE-WMP` + `NOTICE`), distinct from the NJOY attribution.
    pub fn core() -> &'static WmpLibrary {
        static CORE: OnceLock<WmpLibrary> = OnceLock::new();
        CORE.get_or_init(|| {
            WmpLibrary::from_blob(include_bytes!("../data/wmp_core.wmpl"))
                .expect("embedded CORE WMPL blob is valid")
        })
    }

    /// Number of nuclides in the container.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether the container holds no nuclides.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// The nuclide names present, in stored order (e.g. `["U235", "U238", …]`).
    pub fn names(&self) -> Vec<&str> {
        self.index.iter().map(|e| e.name.as_str()).collect()
    }

    /// Whether a nuclide with this name is present.
    pub fn contains(&self, name: &str) -> bool {
        self.index.iter().any(|e| e.name == name)
    }

    /// Decode one nuclide by name. The WMPB blob is inflated on demand and a
    /// fresh [`WindowedMultipole`] returned each call, so a caller that reuses a
    /// nuclide across many evaluations should keep the decoded value.
    ///
    /// # Errors
    /// [`NjoyError::WmpData`] if `name` is absent or its blob fails to decode.
    pub fn get(&self, name: &str) -> Result<WindowedMultipole, NjoyError> {
        let entry =
            self.index.iter().find(|e| e.name == name).ok_or_else(|| {
                NjoyError::WmpData(format!("nuclide {name} not in WMPL container"))
            })?;
        WindowedMultipole::from_blob(&self.bytes[entry.offset..entry.offset + entry.len])
    }
}
