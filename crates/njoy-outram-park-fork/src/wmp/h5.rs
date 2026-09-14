//! [`WindowedMultipole::load_h5`] — reader for the MIT CRPG `WMP_Library`
//! HDF5 files (pure-Rust `hdf5-pure`, no system `libhdf5`).
//!
//! Split out of `wmp.rs` (see the module doc there for provenance and status).

use super::types::{Cf64, WindowedMultipole, WmpWindow};
use crate::NjoyError;

impl WindowedMultipole {
    /// Load a nuclide from an MIT CRPG `WMP_Library` HDF5 file.
    ///
    /// Reads the single nuclide group (e.g. `/U238/`) and its datasets — `data`
    /// (complex poles + residues), `curvefit`, `windows` (1-based → 0-based),
    /// `broaden_poly`, and the `E_min`/`E_max`/`spacing`/`sqrtAWR` scalars — per
    /// the WMP format spec (`WMP_Library/wmp_format.md`). Uses `hdf5-pure`
    /// (pure-Rust, no system `libhdf5`). For the *embedded* offline path (no HDF5
    /// dependency in the shipped build), prefer [`Self::from_blob`].
    ///
    /// **License:** this reads MIT CRPG data —
    /// ship `LICENSE-WMP` + a NOTICE credit before embedding any of it.
    ///
    /// # Errors
    /// [`NjoyError::Io`] if the file cannot be read; [`NjoyError::WmpData`] if a
    /// dataset is missing or has an unexpected shape.
    pub fn load_h5<P: AsRef<std::path::Path>>(path: P) -> Result<Self, NjoyError> {
        use hdf5_pure::File;
        let err = |m: String| NjoyError::WmpData(m);

        let bytes = std::fs::read(path)?;
        let file = File::from_bytes(bytes).map_err(|e| err(format!("open: {e}")))?;

        // The one top-level group is the nuclide (e.g. "U238").
        let name = file
            .root()
            .groups()
            .map_err(|e| err(format!("list groups: {e}")))?
            .into_iter()
            .next()
            .ok_or_else(|| err("no nuclide group in WMP file".into()))?;

        let scalar = |ds: &str| -> Result<f64, NjoyError> {
            let v = file
                .dataset(&format!("/{name}/{ds}"))
                .and_then(|d| d.read_f64())
                .map_err(|e| err(format!("{ds}: {e}")))?;
            v.first().copied().ok_or_else(|| err(format!("{ds} empty")))
        };

        let e_min = scalar("E_min")?;
        let e_max = scalar("E_max")?;
        let spacing = scalar("spacing")?;
        let sqrt_awr = scalar("sqrtAWR")?;

        // `data`: (n_poles, n_cols) complex; col 0 = pole, cols 1.. = residues
        // (scatter, absorption, [fission]). Read raw and decode f64 (r,i) pairs.
        let data_ds = file
            .dataset(&format!("/{name}/data"))
            .map_err(|e| err(format!("data: {e}")))?;
        let shape = data_ds
            .shape()
            .map_err(|e| err(format!("data shape: {e}")))?;
        if shape.len() != 2 {
            return Err(err(format!("data rank {} != 2", shape.len())));
        }
        let (n_poles, n_cols) = (shape[0] as usize, shape[1] as usize);
        let fissionable = n_cols >= 4;
        let raw = data_ds
            .read_raw()
            .map_err(|e| err(format!("data raw: {e}")))?;
        if raw.len() != n_poles * n_cols * 16 {
            return Err(err(format!(
                "data bytes {} != {}",
                raw.len(),
                n_poles * n_cols * 16
            )));
        }
        let cx = |elem: usize| -> Cf64 {
            let b = elem * 16;
            let re = f64::from_le_bytes(raw[b..b + 8].try_into().unwrap());
            let im = f64::from_le_bytes(raw[b + 8..b + 16].try_into().unwrap());
            Cf64::new(re, im)
        };
        let mut poles = Vec::with_capacity(n_poles);
        let mut residues = Vec::with_capacity(n_poles);
        for p in 0..n_poles {
            let base = p * n_cols;
            poles.push(cx(base));
            let scatter = cx(base + 1);
            let absorption = cx(base + 2);
            let fission = if fissionable {
                cx(base + 3)
            } else {
                Cf64::new(0.0, 0.0)
            };
            residues.push([scatter, absorption, fission]);
        }

        // `curvefit`: (n_windows, fit_order+1, 3) f64, row-major.
        let cf_ds = file
            .dataset(&format!("/{name}/curvefit"))
            .map_err(|e| err(format!("curvefit: {e}")))?;
        // Last dim = number of channels: 2 (scatter, absorption) for a
        // non-fissionable nuclide, 3 (+ fission) for a fissionable one.
        let cf_shape = cf_ds
            .shape()
            .map_err(|e| err(format!("curvefit shape: {e}")))?;
        if cf_shape.len() != 3 || (cf_shape[2] != 2 && cf_shape[2] != 3) {
            return Err(err(format!("curvefit shape {cf_shape:?} unexpected")));
        }
        let n_windows = cf_shape[0] as usize;
        let n_coeff = cf_shape[1] as usize;
        let n_ch = cf_shape[2] as usize;
        let fit_order = n_coeff - 1;
        let cf_flat = cf_ds
            .read_f64()
            .map_err(|e| err(format!("curvefit read: {e}")))?;
        let mut curvefit = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let mut coeffs = Vec::with_capacity(n_coeff);
            for c in 0..n_coeff {
                let o = (w * n_coeff + c) * n_ch;
                let fission = if n_ch >= 3 { cf_flat[o + 2] } else { 0.0 };
                coeffs.push([cf_flat[o], cf_flat[o + 1], fission]);
            }
            curvefit.push(coeffs);
        }

        // `windows`: (n_windows, 2) i32, 1-based inclusive pole indices.
        let win_flat = file
            .dataset(&format!("/{name}/windows"))
            .and_then(|d| d.read_i32())
            .map_err(|e| err(format!("windows: {e}")))?;
        // `broaden_poly`: (n_windows,) i8 (1 = broaden the curve-fit).
        let broaden = file
            .dataset(&format!("/{name}/broaden_poly"))
            .and_then(|d| d.read_i8())
            .map_err(|e| err(format!("broaden_poly: {e}")))?;
        if win_flat.len() != 2 * n_windows || broaden.len() != n_windows {
            return Err(err("windows / broaden_poly length mismatch".into()));
        }
        let mut windows = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let (s1, e1) = (win_flat[2 * w], win_flat[2 * w + 1]);
            // Map to 0-based; represent an empty window (e1 < s1) as end < start.
            let (start, end) = if e1 < s1 {
                (1usize, 0usize)
            } else {
                ((s1 - 1) as usize, (e1 - 1) as usize)
            };
            windows.push(WmpWindow {
                start,
                end,
                broaden_poly: broaden[w] != 0,
            });
        }

        Ok(WindowedMultipole {
            name,
            awr: sqrt_awr * sqrt_awr,
            e_min,
            e_max,
            fissionable,
            poles,
            residues,
            curvefit,
            windows,
            inv_spacing: 1.0 / spacing,
            fit_order,
        })
    }
}
