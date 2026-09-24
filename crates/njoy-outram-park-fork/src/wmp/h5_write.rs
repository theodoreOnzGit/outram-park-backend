// SPDX-License-Identifier: GPL-3.0

//! [`WindowedMultipole::write_h5`] — **writer** for the MIT CRPG `WMP_Library`
//! HDF5 format. GitHub #270, scope table row *"WMP library | read (today) +
//! **write**"*.
//!
//! # Why it lives beside the reader rather than in `crate::hdf5`
//!
//! The reader is [`super::h5`]. A reader and a writer of the same format are the
//! pair most likely to drift apart, and the round-trip test that stops them
//! drifting has to see both, so they sit in one module directory. The other
//! OpenMC formats (`mgxs.h5`, the nuclide `.h5`, statepoints) have no
//! pre-existing reader here, which is why those writers live in `crate::hdf5`.
//!
//! # Specification
//!
//! `openmc/data/multipole.py`'s `WindowedMultipole.export_to_hdf5`
//! (`/opt/src/openmc` commit `afa7a14`, MIT), read before writing anything —
//! the layout is not guessed from the reader's inverse:
//!
//! | where | name | type | note |
//! |---|---|---|---|
//! | root attr | `filetype` | ASCII string | `"data_wmp"` |
//! | root attr | `version` | int array | `(1, 1)` = `WMP_VERSION`. **Required** — `from_hdf5` raises `DataError` without it |
//! | `/<name>` | `spacing` | f64 scalar | window spacing in `sqrt(E)` |
//! | `/<name>` | `sqrtAWR` | f64 scalar | `sqrt(awr)`, not `awr` |
//! | `/<name>` | `E_min`, `E_max` | f64 scalar | eV |
//! | `/<name>` | `data` | complex128, `(n_poles, n_cols)` | col 0 pole, then scatter / absorption / [fission] residues |
//! | `/<name>` | `windows` | int, `(n_windows, 2)` | **1-based inclusive** pole indices |
//! | `/<name>` | `broaden_poly` | int8, `(n_windows,)` | read back by OpenMC as `bool` |
//! | `/<name>` | `curvefit` | f64, `(n_windows, fit_order+1, n_ch)` | `n_ch` = 2, or 3 when fissionable |
//!
//! # Two things that would have produced a file OpenMC cannot read
//!
//! 1. **`hdf5-pure`'s `with_complex64_data` helper names its compound fields
//!    `real` / `imag`.** h5py recognises a two-field f64 compound as
//!    `complex128` only when the field names match its `complex_names` config,
//!    whose default is **`('r', 'i')`**. Using the convenience helper writes a
//!    structurally valid file whose `data` h5py hands back as a *structured*
//!    array, not a complex one, and OpenMC's evaluation then fails on
//!    arithmetic rather than on anything that names the real cause. So the
//!    compound is built explicitly with `r` / `i`.
//! 2. **The scalars must be 0-dimensional, not length-1.** `export_to_hdf5`
//!    writes `np.array(float)`, a 0-d array, and `from_hdf5` reads
//!    `group['spacing'][()]`. From a shape-`[1]` dataset that idiom returns
//!    `array([x])` rather than a float, and `spacing` then silently becomes an
//!    array that broadcasts through the whole evaluation. `with_shape(&[])`
//!    gives a true scalar dataspace.
//!
//! Neither is visible from the reader side: [`super::h5`] accepts both
//! spellings, so a round trip through this crate alone would have passed while
//! OpenMC choked. That is the argument for the cross-code test, not just the
//! round-trip one.
//!
//! # Known lossiness, stated rather than discovered later
//!
//! An **empty window** (one with no poles) is stored in memory as
//! `start = 1, end = 0` by [`super::h5`], which discards *where* in the pole
//! list the empty window sat. This writer emits `(1, 0)` for it. OpenMC iterates
//! `range(windows[w,0] - 1, windows[w,1])`, so `(1, 0)` and the original
//! `(k+1, k)` both yield no poles and evaluate identically — but the file is
//! **not byte-identical** to one that came in with a different empty-window
//! spelling. The round trip is lossless in *value*, not in *bytes*, and only for
//! this one field.
//!
//! **License:** the WMP data itself is MIT CRPG — `LICENSE-WMP` and the `NOTICE`
//! credit must travel with any file written from the embedded library.

use std::path::Path;

use hdf5_pure::{AttrValue, CompoundTypeBuilder, FileBuilder};

use super::types::WindowedMultipole;
use crate::NjoyError;

/// `WMP_VERSION` in `openmc/data/__init__.py` — major 1, minor 1.
pub const WMP_VERSION: (i64, i64) = (1, 1);

impl WindowedMultipole {
    /// Write this nuclide to a `WMP_Library`-format HDF5 file.
    ///
    /// The inverse of [`Self::load_h5`], and verified as such: see
    /// `tests/wmp_h5_write_round_trip.rs` for the value round trip and
    /// `tests/wmp_h5_vs_openmc.rs` for OpenMC reading the result back.
    ///
    /// # Errors
    ///
    /// [`NjoyError::WmpData`] if the record is internally inconsistent — an
    /// empty name, no poles, no windows, a residue count that disagrees with
    /// the pole count, or a curve-fit row whose length disagrees with
    /// `fit_order`. These are checked here rather than trusted, because a
    /// malformed file is discovered by whoever reads it next and this is the
    /// last place that can name the cause. [`NjoyError::Hdf5`] if the write
    /// itself fails.
    pub fn write_h5<P: AsRef<Path>>(&self, path: P) -> Result<(), NjoyError> {
        let err = |m: String| NjoyError::WmpData(m);

        if self.name.is_empty() {
            return Err(err("WMP record has no nuclide name; it becomes the \
                            HDF5 group name and cannot be empty"
                .into()));
        }
        if self.poles.is_empty() {
            return Err(err(format!("{}: no poles to write", self.name)));
        }
        if self.residues.len() != self.poles.len() {
            return Err(err(format!(
                "{}: {} poles but {} residue rows",
                self.name,
                self.poles.len(),
                self.residues.len()
            )));
        }
        if self.windows.is_empty() {
            return Err(err(format!("{}: no windows to write", self.name)));
        }
        if self.curvefit.len() != self.windows.len() {
            return Err(err(format!(
                "{}: {} windows but {} curve-fit rows; OpenMC's from_hdf5 \
                 rejects this mismatch too",
                self.name,
                self.windows.len(),
                self.curvefit.len()
            )));
        }
        // OpenMC: "Windowed multipole is only supported for curvefits with 3 or
        // more terms" (`fit_order < 2` is a hard error there).
        if self.fit_order < 2 {
            return Err(err(format!(
                "{}: fit_order {} gives {} curve-fit terms; OpenMC requires at \
                 least 3 and refuses the file",
                self.name,
                self.fit_order,
                self.fit_order + 1
            )));
        }
        let n_coeff = self.fit_order + 1;
        for (w, row) in self.curvefit.iter().enumerate() {
            if row.len() != n_coeff {
                return Err(err(format!(
                    "{}: curve-fit window {w} has {} coefficients, expected \
                     {n_coeff} for fit_order {}",
                    self.name,
                    row.len(),
                    self.fit_order
                )));
            }
        }
        if !self.inv_spacing.is_finite() || self.inv_spacing == 0.0 {
            return Err(err(format!(
                "{}: inv_spacing is {}; `spacing` would not be finite",
                self.name, self.inv_spacing
            )));
        }
        if self.awr < 0.0 {
            return Err(err(format!(
                "{}: awr {} is negative; sqrtAWR would be NaN",
                self.name, self.awr
            )));
        }

        let n_poles = self.poles.len();
        let n_windows = self.windows.len();
        // Residue columns actually written. A non-fissionable nuclide gets 3
        // columns (pole + scatter + absorption); the fission residues are not
        // written at all, matching what `load_h5` expects to find.
        let n_cols = if self.fissionable { 4 } else { 3 };
        let n_ch = if self.fissionable { 3 } else { 2 };

        let mut b = FileBuilder::new();
        b.set_attr("filetype", AttrValue::AsciiString("data_wmp".into()));
        b.set_attr(
            "version",
            AttrValue::I64Array(vec![WMP_VERSION.0, WMP_VERSION.1]),
        );

        let mut g = b.create_group(&self.name);

        // Scalars, 0-dimensional (see the module note: a length-1 dataset makes
        // OpenMC's `[()]` return an array instead of a float).
        for (name, v) in [
            ("spacing", 1.0 / self.inv_spacing),
            ("sqrtAWR", self.awr.sqrt()),
            ("E_min", self.e_min),
            ("E_max", self.e_max),
        ] {
            g.create_dataset(name).with_f64_data(&[v]).with_shape(&[]);
        }

        // `data`: complex128 (n_poles, n_cols), field names `r` / `i` so h5py
        // sees complex rather than a structured array.
        let complex128 = CompoundTypeBuilder::new()
            .f64_field("r")
            .f64_field("i")
            .build();
        let mut raw = Vec::with_capacity(n_poles * n_cols * 16);
        let mut push = |z: super::types::Cf64| {
            raw.extend_from_slice(&z.re.to_le_bytes());
            raw.extend_from_slice(&z.im.to_le_bytes());
        };
        for p in 0..n_poles {
            push(self.poles[p]);
            push(self.residues[p][0]); // scatter
            push(self.residues[p][1]); // absorption
            if self.fissionable {
                push(self.residues[p][2]);
            }
        }
        g.create_dataset("data")
            .with_compound_data(complex128, raw, (n_poles * n_cols) as u64)
            .with_shape(&[n_poles as u64, n_cols as u64]);

        // `windows`: 1-based inclusive. An empty window is written `(1, 0)`,
        // which OpenMC's `range(lo - 1, hi)` treats as no poles — see the
        // module's lossiness note.
        let mut windows = Vec::with_capacity(2 * n_windows);
        for w in &self.windows {
            if w.end < w.start {
                windows.push(1i32);
                windows.push(0i32);
            } else {
                windows.push(w.start as i32 + 1);
                windows.push(w.end as i32 + 1);
            }
        }
        g.create_dataset("windows")
            .with_i32_data(&windows)
            .with_shape(&[n_windows as u64, 2]);

        let broaden: Vec<i8> = self
            .windows
            .iter()
            .map(|w| i8::from(w.broaden_poly))
            .collect();
        g.create_dataset("broaden_poly").with_i8_data(&broaden);

        // `curvefit`: (n_windows, n_coeff, n_ch), row-major — the fission
        // channel is dropped for a non-fissionable nuclide, matching `data`.
        let mut cf = Vec::with_capacity(n_windows * n_coeff * n_ch);
        for row in &self.curvefit {
            for c in row {
                cf.push(c[0]);
                cf.push(c[1]);
                if self.fissionable {
                    cf.push(c[2]);
                }
            }
        }
        g.create_dataset("curvefit")
            .with_f64_data(&cf)
            .with_shape(&[n_windows as u64, n_coeff as u64, n_ch as u64]);

        b.add_group(g.finish());
        b.write(path.as_ref())
            .map_err(|e| NjoyError::Hdf5(format!("writing {}: {e}", self.name)))
    }
}
