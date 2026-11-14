// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Translation of two non-table-interpolation branches of Moltres'
// GenericMoltresMaterial:
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7a60f59424d0d8f46748ba4e243d76b671
//   Upstream sources translated:
//     src/materials/GenericMoltresMaterial.C
//       leastSquaresConstruct()          lines 313-364
//       leastSquaresComputeQpProperties() lines 511-549
//       bicubicSplineConstruct()         lines 243-311
//       fuelBicubic() / moderatorBicubic() lines 366-496
//   and (via crate::moose_interpolation) MOOSE d7c4bde4025537ac4574cc80e78c2407e23a42cc
//   BicubicSplineInterpolation.
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 conversion option.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Moltres `interp_type = least_squares` ([`LsqTables`]) and `interp_type =
//! bicubic` ([`BicubicTables`]) group constants (gh:#1538).
//!
//! **No gold data.** Moltres 3dd2ce7 ships no test input and no gold file
//! for either type (checked: the only `bicubic`/`least_squares` hits in the
//! tree are the sources themselves), so these two are verified at kernel
//! level only (the C++ transcription in
//! `verification_and_validation/moltres_differential_gh1400/`), never
//! against a Moltres solution. They are untested against any real Moltres
//! input file.
//!
//! **Deliberate deviations from upstream** (upstream is silent where this
//! returns an error): a missing or short file is an
//! [`MoltresError::InvalidMaterial`]. Upstream's `leastSquaresConstruct`
//! ignores a file that cannot be opened and leaves every coefficient zero,
//! and `myfile >> value` on a short file silently reuses the last value.

use crate::error::MoltresError;
use crate::moose_interpolation::BicubicSplineInterpolation;
use crate::materials::MsrMaterial;
use crate::tables::{material_from_samples, TableLayout};

/// Moltres' `_xsec_names` order (`NuclearMaterial.h`), which is also the
/// record order of the least-squares file.
const XSEC_NAMES: [&str; 12] = [
    "REMXS",
    "FISSXS",
    "NSF",
    "FISSE",
    "DIFFCOEF",
    "RECIPVEL",
    "CHI_T",
    "CHI_P",
    "CHI_D",
    "GTRANSFXS",
    "BETA_EFF",
    "DECAY_CONSTANT",
];

fn vec_length(name: &str, g: usize, p: usize) -> usize {
    match name {
        "GTRANSFXS" => g * g,
        "BETA_EFF" | "DECAY_CONSTANT" => p,
        _ => g,
    }
}

/// `interp_type = least_squares`: every constant is `c0 * T + c1`, read
/// from ONE file at `property_tables_root` as `c0 c1` pairs in
/// `_xsec_names` order, entry by entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LsqTables {
    /// Number of neutron groups.
    pub groups: usize,
    /// Number of precursor groups.
    pub precursor_groups: usize,
    /// Scatter index layout (`sss2_input`).
    pub layout: TableLayout,
    /// `coeffs[n][j] = (c0, c1)` for name `XSEC_NAMES[n]`, entry `j`.
    coeffs: Vec<Vec<(f64, f64)>>,
}

impl LsqTables {
    /// Parse the single least-squares file.
    ///
    /// # Errors
    /// Unparseable token, or fewer numbers than `2 * sum(entries)`.
    pub fn from_text(
        text: &str,
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
    ) -> Result<Self, MoltresError> {
        let mut it = text.split_whitespace();
        let mut coeffs = Vec::with_capacity(XSEC_NAMES.len());
        for name in XSEC_NAMES {
            let n = vec_length(name, groups, precursor_groups);
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                let mut pair = [0.0; 2];
                for slot in &mut pair {
                    let tok = it.next().ok_or_else(|| {
                        MoltresError::InvalidMaterial(format!("least-squares file: ran out of numbers in {name}"))
                    })?;
                    *slot = tok.parse().map_err(|_| {
                        MoltresError::InvalidMaterial(format!("least-squares file: cannot parse '{tok}'"))
                    })?;
                }
                v.push((pair[0], pair[1]));
            }
            coeffs.push(v);
        }
        Ok(Self {
            groups,
            precursor_groups,
            layout,
            coeffs,
        })
    }

    /// Read the file at `path`.
    ///
    /// # Errors
    /// Unreadable file, or as [`Self::from_text`].
    pub fn load(
        path: &str,
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
    ) -> Result<Self, MoltresError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| MoltresError::InvalidMaterial(format!("cannot read {path}: {e}")))?;
        Self::from_text(&text, groups, precursor_groups, layout)
    }

    /// `(value, d value / dT)` of constant `name` entry `k` at `t`, in
    /// Moltres' own units: `c0 * t + c1` and `c0`
    /// (`leastSquaresComputeQpProperties`). `None` for an unknown name.
    #[must_use]
    pub fn raw_sample(&self, name: &str, k: usize, t: f64) -> Option<(f64, f64)> {
        let n = XSEC_NAMES.iter().position(|x| *x == name)?;
        let (c0, c1) = *self.coeffs[n].get(k)?;
        Some((c0 * t + c1, c0))
    }

    /// SI [`MsrMaterial`] at `t`, same conversions as
    /// [`crate::tables::MoltresTables::material_at`].
    #[must_use]
    pub fn material_at(&self, name: &str, t: f64) -> MsrMaterial {
        material_from_samples(name, self.groups, self.layout, |c, k| {
            self.raw_sample(c, k, t).unwrap_or((f64::NAN, f64::NAN))
        })
    }
}

/// Which material a [`BicubicTables`] describes: it decides which argument
/// is the local temperature (`material = fuel | moderator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BicubicMaterial {
    /// Local temperature is `x1` (the fuel axis); slope is `d/dx1`.
    Fuel,
    /// Local temperature is `x2` (the moderator axis); slope is `d/dx2`.
    Moderator,
}

/// `interp_type = bicubic`: eleven two-variable tables on a
/// `fuel_temp_points x mod_temp_points` grid, interpolated with MOOSE's
/// `BicubicSplineInterpolation`.
///
/// File layout (`bicubicSplineConstruct`): for each fuel point `h`, for each
/// moderator point `i`, one line `T_fuel T_mod v_1 ... v_n` (the two
/// temperatures are skipped, not checked against the points given in the
/// input).
#[derive(Debug, Clone, PartialEq)]
pub struct BicubicTables {
    /// Number of neutron groups.
    pub groups: usize,
    /// Number of precursor groups.
    pub precursor_groups: usize,
    /// Scatter index layout (`sss2_input`).
    pub layout: TableLayout,
    /// Fuel or moderator.
    pub material: BicubicMaterial,
    /// `fits[c][k]` for the constant `c` in [`crate::tables::file_suffixes`]
    /// order (REMXS, NSF, DIFFCOEF, BETA_EFF, FISSXS, FISSE, CHI_T, CHI_P,
    /// CHI_D, GTRANSFXS, DECAY_CONSTANT) and entry `k`.
    fits: Vec<Vec<BicubicSplineInterpolation>>,
}

const BICUBIC_ORDER: [&str; 11] = [
    "REMXS",
    "NSF",
    "DIFFCOEF",
    "BETA_EFF",
    "FISSXS",
    "FISSE",
    "CHI_T",
    "CHI_P",
    "CHI_D",
    "GTRANSFXS",
    "DECAY_CONSTANT",
];

impl BicubicTables {
    /// Build from the eleven file texts in [`crate::tables::file_suffixes`]
    /// order.
    ///
    /// # Errors
    /// Short or unparseable file, or fewer than two points on an axis.
    #[allow(clippy::too_many_arguments)]
    pub fn from_texts(
        texts: &[&str; 11],
        fuel_points: &[f64],
        mod_points: &[f64],
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
        material: BicubicMaterial,
    ) -> Result<Self, MoltresError> {
        if fuel_points.is_empty() {
            return Err(MoltresError::InvalidMaterial(
                "You forgot to supply *fuel_temp_points* for interpolation!".into(),
            ));
        }
        if mod_points.is_empty() {
            return Err(MoltresError::InvalidMaterial(
                "You forgot to supply *mod_temp_points* for interpolation!".into(),
            ));
        }
        let (l, m) = (fuel_points.len(), mod_points.len());
        let mut fits = Vec::with_capacity(11);
        for (c, text) in texts.iter().enumerate() {
            let name = BICUBIC_ORDER[c];
            let o = vec_length(name, groups, precursor_groups);
            let mut it = text.split_whitespace();
            let mut next = || -> Result<f64, MoltresError> {
                let tok = it.next().ok_or_else(|| {
                    MoltresError::InvalidMaterial(format!("bicubic table {name}: file is too short"))
                })?;
                tok.parse().map_err(|_| {
                    MoltresError::InvalidMaterial(format!("bicubic table {name}: cannot parse '{tok}'"))
                })
            };
            // data[k][h][i]
            let mut data = vec![vec![Vec::with_capacity(m); l]; o];
            for h in 0..l {
                for _ in 0..m {
                    next()?;
                    next()?;
                    for col in data.iter_mut() {
                        col[h].push(next()?);
                    }
                }
            }
            let mut per_k = Vec::with_capacity(o);
            for col in &data {
                per_k.push(BicubicSplineInterpolation::new(fuel_points, mod_points, col)?);
            }
            fits.push(per_k);
        }
        Ok(Self {
            groups,
            precursor_groups,
            layout,
            material,
            fits,
        })
    }

    /// Read `<root><SUFFIX>.txt` for the eleven constants.
    ///
    /// # Errors
    /// Unreadable file, or as [`Self::from_texts`].
    pub fn load(
        root: &str,
        fuel_points: &[f64],
        mod_points: &[f64],
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
        material: BicubicMaterial,
    ) -> Result<Self, MoltresError> {
        let names = crate::tables::file_suffixes(layout);
        let mut texts: Vec<String> = Vec::with_capacity(11);
        for n in names {
            let path = format!("{root}{n}.txt");
            texts.push(std::fs::read_to_string(&path).map_err(|e| {
                MoltresError::InvalidMaterial(format!("cannot read {path}: {e}"))
            })?);
        }
        let refs: [&str; 11] = std::array::from_fn(|i| texts[i].as_str());
        Self::from_texts(
            &refs,
            fuel_points,
            mod_points,
            groups,
            precursor_groups,
            layout,
            material,
        )
    }

    /// `(value, d value / d t_self)` of the constant `name` entry `k`
    /// (names as in [`crate::tables::MoltresTables::raw_sample`]), `t_self` the local
    /// temperature and `t_other` the other material's average temperature
    /// (`other_temp`). `None` for an unknown name or entry.
    #[must_use]
    pub fn raw_sample(&self, name: &str, k: usize, t_self: f64, t_other: f64) -> Option<(f64, f64)> {
        let c = BICUBIC_ORDER.iter().position(|x| *x == name)?;
        let f = self.fits[c].get(k)?;
        Some(match self.material {
            BicubicMaterial::Fuel => (f.sample(t_self, t_other), f.sample_derivative(t_self, t_other, 1)),
            BicubicMaterial::Moderator => {
                (f.sample(t_other, t_self), f.sample_derivative(t_other, t_self, 2))
            }
        })
    }

    /// SI [`MsrMaterial`] at the local temperature `t_self` and the other
    /// material's `t_other`.
    #[must_use]
    pub fn material_at(&self, name: &str, t_self: f64, t_other: f64) -> MsrMaterial {
        material_from_samples(name, self.groups, self.layout, |c, k| {
            self.raw_sample(c, k, t_self, t_other)
                .unwrap_or((f64::NAN, f64::NAN))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lsq_text(g: usize, p: usize) -> String {
        // every entry (c0, c1) = (0.001 * n, 1 + n) with n a running index
        let mut s = String::new();
        let mut n = 0.0;
        for name in XSEC_NAMES {
            for _ in 0..vec_length(name, g, p) {
                s.push_str(&format!("{} {}\n", 0.001 * n, 1.0 + n));
                n += 1.0;
            }
        }
        s
    }

    #[test]
    fn lsq_is_c0_t_plus_c1_with_slope_c0() {
        let t = LsqTables::from_text(&lsq_text(2, 2), 2, 2, TableLayout::Legacy).unwrap();
        // REMXS entry 0 is running index 0, entry 1 is index 1
        assert_eq!(t.raw_sample("REMXS", 1, 900.0), Some((0.001 * 900.0 + 2.0, 0.001)));
        // FISSXS starts at index 2
        assert_eq!(t.raw_sample("FISSXS", 0, 10.0), Some((0.002 * 10.0 + 3.0, 0.002)));
        assert!(t.raw_sample("NOPE", 0, 1.0).is_none());
        assert!(t.raw_sample("REMXS", 9, 1.0).is_none());
        let m = t.material_at("m", 900.0);
        assert_eq!(m.sigma_removal.len(), 2);
        assert_eq!(m.d_sigma_removal_d_temp[1], 0.001 * 100.0);
        assert!(LsqTables::from_text("1 2 3", 2, 2, TableLayout::Legacy).is_err());
        assert!(LsqTables::from_text("x", 2, 2, TableLayout::Legacy).is_err());
        assert!(LsqTables::load("/nonexistent/lsq.txt", 2, 2, TableLayout::Legacy).is_err());
    }

    /// One-group, one-precursor bicubic file for `f(Tf, Tm) = a + b Tf + c Tm`.
    fn bicubic_texts(fp: &[f64], mp: &[f64]) -> Vec<String> {
        let lens = [1usize, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
        let mut out = Vec::new();
        for (ci, o) in lens.iter().enumerate() {
            let mut s = String::new();
            for tf in fp {
                for tm in mp {
                    s.push_str(&format!("{tf} {tm}"));
                    for _ in 0..*o {
                        s.push_str(&format!(" {}", (ci as f64 + 1.0) + 0.01 * tf + 0.002 * tm));
                    }
                    s.push('\n');
                }
            }
            out.push(s);
        }
        out
    }

    #[test]
    fn bicubic_fuel_and_moderator_pick_the_right_axis() {
        let fp = [800.0, 900.0, 1000.0];
        let mp = [700.0, 800.0, 900.0, 1000.0];
        let texts = bicubic_texts(&fp, &mp);
        let refs: [&str; 11] = std::array::from_fn(|i| texts[i].as_str());
        let fuel = BicubicTables::from_texts(&refs, &fp, &mp, 1, 1, TableLayout::Legacy, BicubicMaterial::Fuel)
            .unwrap();
        let moder =
            BicubicTables::from_texts(&refs, &fp, &mp, 1, 1, TableLayout::Legacy, BicubicMaterial::Moderator)
                .unwrap();
        // REMXS = 1 + 0.01 Tf + 0.002 Tm; planar, so the spline is exact
        let (v, d) = fuel.raw_sample("REMXS", 0, 850.0, 750.0).unwrap();
        assert!((v - (1.0 + 8.5 + 1.5)).abs() < 1e-9 && (d - 0.01).abs() < 1e-9);
        let (v, d) = moder.raw_sample("REMXS", 0, 750.0, 850.0).unwrap();
        assert!((v - (1.0 + 8.5 + 1.5)).abs() < 1e-9 && (d - 0.002).abs() < 1e-9);
        assert!(fuel.raw_sample("NOPE", 0, 1.0, 1.0).is_none());
        let m = fuel.material_at("fuel", 850.0, 750.0);
        assert_eq!(m.sigma_removal.len(), 1);
        assert!(BicubicTables::load("/nonexistent/x_", &fp, &mp, 1, 1, TableLayout::Legacy, BicubicMaterial::Fuel).is_err());
        assert!(BicubicTables::from_texts(&refs, &[], &mp, 1, 1, TableLayout::Legacy, BicubicMaterial::Fuel).is_err());
        assert!(BicubicTables::from_texts(&refs, &fp, &[], 1, 1, TableLayout::Legacy, BicubicMaterial::Fuel).is_err());
        let short: [&str; 11] = std::array::from_fn(|_| "800 700 1.0");
        assert!(BicubicTables::from_texts(&short, &fp, &mp, 1, 1, TableLayout::Legacy, BicubicMaterial::Fuel).is_err());
        let bad: [&str; 11] = std::array::from_fn(|_| "x");
        assert!(BicubicTables::from_texts(&bad, &fp, &mp, 1, 1, TableLayout::Legacy, BicubicMaterial::Fuel).is_err());
    }
}
