// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Translation of Moltres' temperature-table group-constant path
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7a60f59424d0d8f46748ba4e243d76b671
//   Upstream sources translated:
//     src/materials/GenericMoltresMaterial.C  (file-name map, Construct())
//     src/materials/NuclearMaterial.C         (dummy/linear ComputeQpProperties,
//                                              the MeV -> J factor 1e6 * 1.6e-19)
//   and the MOOSE framework class it delegates to:
//     framework/src/utils/LinearInterpolation.C  (MOOSE commit d7c4bde,
//                                              sample / sampleDerivative)
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 conversion option. No MOOSE or libMesh is linked.
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

//! Moltres temperature-table group constants: reader and linear
//! interpolation (`GenericMoltresMaterial`, `interp_type = linear | none`).
//!
//! Moltres reads one text file per constant, each line `T v_1 ... v_n`, and
//! interpolates every constant in the local temperature. This module
//! reproduces the file layout (both the Serpent-2 `sss2_input = true` names
//! and the legacy names), the `interp_type = none` / `linear` evaluation and
//! the temperature derivative Moltres feeds to its Jacobian. **Spline,
//! monotone-cubic interpolation are translated in
//! [`crate::moose_interpolation`] and wired in here (gh:#1538); bicubic and
//! least-squares tables, whose data shapes differ, are in
//! [`crate::tables_multivar`].** ~~Spline, monotone-cubic, bicubic and
//! least-squares interpolation are not translated~~ **CORRECTED 2026-11-13**
//! (gh:#1538).
//!
//! # Units
//!
//! Moltres tables are in **cm** (`D` in cm, `Sigma` in 1/cm). This crate is
//! SI, so [`MoltresTables::material_at`] converts: `D / 100`,
//! `Sigma * 100`. `FISSE` is MeV per fission and becomes joules with
//! Moltres' own factor `1e6 * 1.6e-19`; `sigma_power = FISSE_J * FISSXS`
//! in J/m.
//!
//! # Index layout of `GTRANSFXS` / `SP0`
//!
//! `InScatter` selects the entry for source group `i` into group `g` as
//! `i * G + g` when `sss2_input` is true and `i + g * G` otherwise. The same
//! two layouts are used here; the diagonal (self-scatter) is dropped because
//! Moltres' `InScatter` skips `i == g` and `REMXS` already excludes it.

use crate::error::MoltresError;
use crate::materials::{DelayedFamily, MsrMaterial};
use crate::moose_interpolation::{MonotoneCubicInterpolation, SplineInterpolation};

/// MOOSE `LinearInterpolation::sample` with `extrap = false` for one table
/// column: the end values are held outside `[x_0, x_last)`.
///
/// `x` must be strictly increasing and the same length as `y`. A NaN `v`
/// returns NaN, as upstream does.
#[must_use]
pub fn linear_sample(x: &[f64], y: &[f64], v: f64) -> f64 {
    if x.len() == 1 {
        return y[0];
    }
    if v.is_nan() {
        return f64::NAN;
    }
    if v < x[0] {
        return y[0];
    }
    if v >= x[x.len() - 1] {
        return y[y.len() - 1];
    }
    let upper = x.partition_point(|xi| *xi <= v);
    let i = upper - 1;
    if i == x.len() - 1 {
        return f64::NAN;
    }
    y[i] + (y[i + 1] - y[i]) * (v - x[i]) / (x[i + 1] - x[i])
}

/// MOOSE `LinearInterpolation::sampleDerivative` with `extrap = false`: the
/// piecewise slope inside `[x_0, x_last)`, and `0` outside it (and for a
/// single-point table, where upstream would read past the vector).
#[must_use]
pub fn linear_sample_derivative(x: &[f64], y: &[f64], v: f64) -> f64 {
    if v.is_nan() {
        return f64::NAN;
    }
    if x.len() < 2 || v < x[0] || v >= x[x.len() - 1] {
        return 0.0;
    }
    let upper = x.partition_point(|xi| *xi <= v);
    let i = upper - 1;
    if i == x.len() - 1 {
        return f64::NAN;
    }
    (y[i + 1] - y[i]) / (x[i + 1] - x[i])
}

/// `interp_type` of `GenericMoltresMaterial` that this crate translates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpType {
    /// `interp_type = none`: one temperature row only, no dependence.
    None,
    /// `interp_type = linear`: piecewise-linear in temperature.
    Linear,
    /// `interp_type = spline`: MOOSE `SplineInterpolation`, natural cubic
    /// spline; the end cells are extrapolated (gh:#1538).
    Spline,
    /// `interp_type = monotone_cubic`: MOOSE `MonotoneCubicInterpolation`,
    /// at least three rows; the end cells are extrapolated (gh:#1538).
    MonotoneCubic,
}

/// Which of the two Moltres file-name / index conventions the tables use
/// (`sss2_input`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableLayout {
    /// `sss2_input = true`: files `FISS KAPPA INVV CHIT CHIP CHID SP0
    /// LAMBDA`, scatter index `from * G + to`.
    Serpent2,
    /// `sss2_input = false`: files `FISSXS FISSE RECIPVEL CHI CHI CHI_D
    /// GTRANSFXS DECAY_CONSTANT`, scatter index `from + to * G`.
    Legacy,
}

/// File-name suffixes for one constant under a [`TableLayout`], in the order
/// `[REMXS, NSF, DIFFCOEF, BETA_EFF, FISSXS, FISSE, CHI_T, CHI_P, CHI_D,
/// GTRANSFXS, DECAY_CONSTANT]` (Moltres `_file_map`).
#[must_use]
pub fn file_suffixes(layout: TableLayout) -> [&'static str; 11] {
    match layout {
        TableLayout::Serpent2 => [
            "REMXS", "NSF", "DIFFCOEF", "BETA_EFF", "FISS", "KAPPA", "CHIT", "CHIP", "CHID", "SP0",
            "LAMBDA",
        ],
        TableLayout::Legacy => [
            "REMXS",
            "NSF",
            "DIFFCOEF",
            "BETA_EFF",
            "FISSXS",
            "FISSE",
            "CHI",
            "CHI",
            "CHI_D",
            "GTRANSFXS",
            "DECAY_CONSTANT",
        ],
    }
}

/// One Moltres property file: a temperature axis and `n` value columns.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyTable {
    /// Strictly increasing temperatures (K).
    pub temperature: Vec<f64>,
    /// `columns[k][i]` = value `k` at `temperature[i]`.
    pub columns: Vec<Vec<f64>>,
    /// Built by [`Self::prepared`] for [`InterpType::Spline`].
    splines: Vec<SplineInterpolation>,
    /// Built by [`Self::prepared`] for [`InterpType::MonotoneCubic`].
    monotones: Vec<MonotoneCubicInterpolation>,
}

impl PropertyTable {
    /// Parse the text of one property file with `n_values` values per line.
    ///
    /// Mirrors `GenericMoltresMaterial::Construct`: whitespace-separated
    /// numbers, temperatures must not decrease, the count must be a whole
    /// number of lines. (Upstream tolerates equal consecutive temperatures
    /// in the read but `LinearInterpolation` then rejects them; this reader
    /// rejects them at parse time.)
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] naming the first problem found.
    pub fn parse(name: &str, text: &str, n_values: usize) -> Result<Self, MoltresError> {
        let mut nums = Vec::new();
        for tok in text.split_whitespace() {
            let v: f64 = tok.parse().map_err(|_| {
                MoltresError::InvalidMaterial(format!("table {name}: cannot parse '{tok}'"))
            })?;
            nums.push(v);
        }
        let stride = n_values + 1;
        if nums.is_empty() || nums.len() % stride != 0 {
            return Err(MoltresError::InvalidMaterial(format!(
                "table {name}: {} numbers is not a whole number of rows of 1 + {n_values}",
                nums.len()
            )));
        }
        let rows = nums.len() / stride;
        let mut temperature = Vec::with_capacity(rows);
        let mut columns = vec![Vec::with_capacity(rows); n_values];
        for r in 0..rows {
            let t = nums[r * stride];
            if let Some(prev) = temperature.last() {
                if t <= *prev {
                    return Err(MoltresError::InvalidMaterial(format!(
                        "table {name}: temperatures are not strictly increasing"
                    )));
                }
            }
            temperature.push(t);
            for (k, col) in columns.iter_mut().enumerate() {
                col.push(nums[r * stride + 1 + k]);
            }
        }
        Ok(Self::from_columns(temperature, columns))
    }

    /// A table from its temperature axis and value columns, with no
    /// spline caches built yet.
    #[must_use]
    pub fn from_columns(temperature: Vec<f64>, columns: Vec<Vec<f64>>) -> Self {
        Self {
            temperature,
            columns,
            splines: Vec::new(),
            monotones: Vec::new(),
        }
    }

    /// Build the per-column interpolators `interp` needs, as Moltres'
    /// `Construct` does (`setData` on each column). `None` and `Linear`
    /// need none. Without this call [`Self::value`] still works for spline
    /// and monotone-cubic, rebuilding the interpolator on each call.
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] when MOOSE's `errorCheck` would
    /// throw, and for monotone cubic with fewer than three rows (Moltres'
    /// own `mooseError`).
    pub fn prepared(mut self, name: &str, interp: InterpType) -> Result<Self, MoltresError> {
        self.splines.clear();
        self.monotones.clear();
        match interp {
            InterpType::None | InterpType::Linear => {}
            InterpType::Spline => {
                for col in &self.columns {
                    self.splines
                        .push(SplineInterpolation::new(&self.temperature, col)?);
                }
            }
            InterpType::MonotoneCubic => {
                if self.temperature.len() < 3 {
                    return Err(MoltresError::InvalidMaterial(format!(
                        "table {name}: Monotone cubic interpolation requires at least three data points."
                    )));
                }
                for col in &self.columns {
                    self.monotones
                        .push(MonotoneCubicInterpolation::new(&self.temperature, col)?);
                }
            }
        }
        Ok(self)
    }

    /// Value `k` at `t` for the given [`InterpType`].
    #[must_use]
    pub fn value(&self, k: usize, t: f64, interp: InterpType) -> f64 {
        match interp {
            InterpType::None => self.columns[k][0],
            InterpType::Linear => linear_sample(&self.temperature, &self.columns[k], t),
            InterpType::Spline => match self.splines.get(k) {
                Some(s) => s.sample(t),
                None => SplineInterpolation::new(&self.temperature, &self.columns[k])
                    .map_or(f64::NAN, |s| s.sample(t)),
            },
            InterpType::MonotoneCubic => match self.monotones.get(k) {
                Some(s) => s.sample(t),
                None => MonotoneCubicInterpolation::new(&self.temperature, &self.columns[k])
                    .map_or(f64::NAN, |s| s.sample(t)),
            },
        }
    }

    /// `d value_k / dT` at `t` (Moltres `d_*_d_temp`).
    #[must_use]
    pub fn slope(&self, k: usize, t: f64, interp: InterpType) -> f64 {
        match interp {
            InterpType::None => 0.0,
            InterpType::Linear => linear_sample_derivative(&self.temperature, &self.columns[k], t),
            InterpType::Spline => match self.splines.get(k) {
                Some(s) => s.sample_derivative(t),
                None => SplineInterpolation::new(&self.temperature, &self.columns[k])
                    .map_or(f64::NAN, |s| s.sample_derivative(t)),
            },
            InterpType::MonotoneCubic => match self.monotones.get(k) {
                Some(s) => s.sample_derivative(t),
                None => MonotoneCubicInterpolation::new(&self.temperature, &self.columns[k])
                    .map_or(f64::NAN, |s| s.sample_derivative(t)),
            },
        }
    }
}

/// The full set of Moltres property tables for one material.
#[derive(Debug, Clone, PartialEq)]
pub struct MoltresTables {
    /// Number of neutron energy groups `G`.
    pub groups: usize,
    /// Number of delayed-neutron precursor groups `P`.
    pub precursor_groups: usize,
    /// Index layout of the scatter matrix.
    pub layout: TableLayout,
    /// Interpolation applied by [`Self::material_at`].
    pub interp: InterpType,
    remxs: PropertyTable,
    nsf: PropertyTable,
    diffcoef: PropertyTable,
    beta_eff: PropertyTable,
    fissxs: PropertyTable,
    fisse: PropertyTable,
    chi_t: PropertyTable,
    chi_p: PropertyTable,
    chi_d: PropertyTable,
    gtransfxs: PropertyTable,
    decay: PropertyTable,
}

/// Moltres' MeV -> J factor `1e6 * 1.6e-19` (`NuclearMaterial.C`). **Do not
/// multiply by this folded constant when matching Moltres**: upstream writes
/// `X * 1e6 * 1.6e-19`, i.e. `(X * 1e6) * 1.6e-19`, which differs from
/// `X * (1e6 * 1.6e-19)` by 1 ulp in ~28 % of cases (gh:#1538, K13: 2950 of
/// 4096 identical). ~~`material_at` multiplied by this constant~~ **CORRECTED
/// 2026-11-13**: it now applies the two factors in upstream order.
pub const MEV_TO_JOULE_MOLTRES: f64 = 1e6 * 1.6e-19;

/// Assemble the SI [`MsrMaterial`] from a sampler of Moltres' raw
/// (cm, MeV) constants: `sample("REMXS", k)` returns `(value, d value / dT)`
/// of column `k`, for the constant names of [`MoltresTables::raw_sample`].
/// Shared by the file, JSON, least-squares and bicubic paths so the unit
/// conversion exists once (cm -> m, MeV -> J with Moltres' own
/// `1e6 * 1.6e-19`).
pub(crate) fn material_from_samples<F: Fn(&str, usize) -> (f64, f64)>(
    name: &str,
    groups: usize,
    layout: TableLayout,
    sample: F,
) -> MsrMaterial {
    let g = groups;
    let col = |c: &str, scale: f64| -> Vec<f64> { (0..g).map(|k| sample(c, k).0 * scale).collect() };
    let mut scattering = vec![vec![0.0; g]; g];
    for from in 0..g {
        for to in 0..g {
            if from != to {
                let idx = match layout {
                    TableLayout::Serpent2 => from * g + to,
                    TableLayout::Legacy => from + to * g,
                };
                scattering[from][to] = sample("GTRANSFXS", idx).0 * 100.0;
            }
        }
    }
    let sigma_power = (0..g)
        .map(|k| (sample("FISSE", k).0 * 1e6 * 1.6e-19) * (sample("FISSXS", k).0 * 100.0))
        .collect();
    MsrMaterial {
        name: name.into(),
        diffusion: (0..g).map(|k| sample("DIFFCOEF", k).0 / 100.0).collect(),
        sigma_removal: col("REMXS", 100.0),
        nu_sigma_f: col("NSF", 100.0),
        chi_prompt: col("CHI_P", 1.0),
        chi_delayed: col("CHI_D", 1.0),
        scattering,
        sigma_power,
        d_sigma_removal_d_temp: (0..g).map(|k| sample("REMXS", k).1 * 100.0).collect(),
    }
}

impl MoltresTables {
    /// Build from the eleven file texts, in the order of [`file_suffixes`].
    /// `chi_d_text = None` reproduces Moltres' backward-compatibility
    /// branch: delayed neutrons are born in the top group.
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] for a malformed table, or for
    /// `InterpType::None` with more than one temperature row (Moltres
    /// rejects that too).
    pub fn from_texts(
        texts: &[&str; 11],
        chi_d_text: Option<&str>,
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
        interp: InterpType,
    ) -> Result<Self, MoltresError> {
        let names = file_suffixes(layout);
        let g = groups;
        let p = precursor_groups;
        let lens = [g, g, g, p, g, g, g, g, g, g * g, p];
        let mut t: Vec<PropertyTable> = Vec::with_capacity(11);
        for i in 0..11 {
            if i == 8 {
                if let Some(txt) = chi_d_text {
                    t.push(PropertyTable::parse(names[i], txt, lens[i])?);
                } else {
                    let rows = t[0].temperature.clone();
                    let mut cols = vec![vec![0.0; rows.len()]; g];
                    cols[0] = vec![1.0; rows.len()];
                    t.push(PropertyTable::from_columns(rows, cols));
                }
            } else {
                t.push(PropertyTable::parse(names[i], texts[i], lens[i])?);
            }
        }
        if interp == InterpType::None && t.iter().any(|tab| tab.temperature.len() > 1) {
            return Err(MoltresError::InvalidMaterial(
                "interp_type=none needs a single temperature row in every table".into(),
            ));
        }
        Self::from_property_tables(t, groups, precursor_groups, layout, interp)
    }

    /// Assemble from the eleven tables in the order of [`file_suffixes`],
    /// building the interpolators `interp` needs.
    ///
    /// # Errors
    /// As [`PropertyTable::prepared`].
    pub fn from_property_tables(
        t: Vec<PropertyTable>,
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
        interp: InterpType,
    ) -> Result<Self, MoltresError> {
        let names = file_suffixes(layout);
        let mut prepared = Vec::with_capacity(11);
        for (i, tab) in t.into_iter().enumerate() {
            prepared.push(tab.prepared(names[i], interp)?);
        }
        let mut it = prepared.into_iter();
        let mut next = || it.next().expect("eleven tables");
        Ok(Self {
            groups,
            precursor_groups,
            layout,
            interp,
            remxs: next(),
            nsf: next(),
            diffcoef: next(),
            beta_eff: next(),
            fissxs: next(),
            fisse: next(),
            chi_t: next(),
            chi_p: next(),
            chi_d: next(),
            gtransfxs: next(),
            decay: next(),
        })
    }

    /// Read the eleven files `<root><SUFFIX>.txt` from disk (Moltres
    /// `property_tables_root`). A missing `CHI_D` file is allowed, as
    /// upstream.
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] for an unreadable file or a bad
    /// table.
    pub fn load(
        root: &str,
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
        interp: InterpType,
    ) -> Result<Self, MoltresError> {
        let names = file_suffixes(layout);
        let mut texts: Vec<String> = Vec::with_capacity(11);
        let mut chi_d: Option<String> = None;
        for (i, n) in names.iter().enumerate() {
            let path = format!("{root}{n}.txt");
            match std::fs::read_to_string(&path) {
                Ok(s) => {
                    if i == 8 {
                        chi_d = Some(s.clone());
                    }
                    texts.push(s);
                }
                Err(e) => {
                    if i == 8 {
                        texts.push(String::new());
                    } else {
                        return Err(MoltresError::InvalidMaterial(format!(
                            "cannot read {path}: {e}"
                        )));
                    }
                }
            }
        }
        let refs: [&str; 11] = std::array::from_fn(|i| texts[i].as_str());
        Self::from_texts(
            &refs,
            chi_d.as_deref(),
            groups,
            precursor_groups,
            layout,
            interp,
        )
    }

    /// Moltres `MoltresJsonMaterial`: read the constants from the JSON text
    /// of an `xsdata.json` file (`base_file`) under `material_key`.
    ///
    /// Behaviours of `MoltresJsonMaterial::MoltresJsonMaterial` and
    /// `Construct` kept on purpose: the temperatures are
    /// `xs_root[key]["temp"]` read with `get<int>()` (truncated towards
    /// zero) and each row is looked up under `to_string(int(T))`; a constant
    /// not named in `group_constants` is all zeros; a missing or empty
    /// `CHI_D` becomes `[1, 0, ...]` at every temperature (Moltres warns);
    /// any other missing constant is an error; `interp_type = none` with
    /// more than one temperature is an error; monotone cubic needs three.
    /// `group_constants = None` is Moltres' default (all twelve). The keys
    /// inside the JSON are the Moltres names themselves (`REMXS`, `CHI_P`,
    /// ...), not the Serpent-2 file suffixes. `RECIPVEL` is not read: the
    /// port does not use it.
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] for malformed JSON, a missing key
    /// or a length mismatch, as upstream's `mooseError`.
    pub fn from_json(
        json_text: &str,
        material_key: &str,
        groups: usize,
        precursor_groups: usize,
        layout: TableLayout,
        interp: InterpType,
        group_constants: Option<&[&str]>,
    ) -> Result<Self, MoltresError> {
        let err = |m: String| MoltresError::InvalidMaterial(format!("MoltresJsonMaterial: {m}"));
        let root: serde_json::Value =
            serde_json::from_str(json_text).map_err(|e| err(format!("bad JSON: {e}")))?;
        let mat = root
            .get(material_key)
            .ok_or_else(|| err(format!("no material key '{material_key}'")))?;
        let temp_arr = mat
            .get("temp")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| err(format!("'{material_key}/temp' is not an array")))?;
        let mut temps: Vec<f64> = Vec::with_capacity(temp_arr.len());
        for v in temp_arr {
            let x = v
                .as_f64()
                .ok_or_else(|| err("a 'temp' entry is not a number".into()))?;
            // nlohmann `get<int>()` is a static_cast: truncation toward zero
            temps.push(x.trunc());
        }
        let l = temps.len();
        if interp == InterpType::None && l > 1 {
            return Err(err(format!(
                "Group constant data for '{material_key}' provided at multiple temperatures with interp_type=none"
            )));
        }
        let all = [
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
        let wanted: Vec<&str> = group_constants.map_or_else(|| all.to_vec(), <[&str]>::to_vec);
        let g = groups;
        let p = precursor_groups;
        // [REMXS, NSF, DIFFCOEF, BETA_EFF, FISSXS, FISSE, CHI_T, CHI_P, CHI_D,
        //  GTRANSFXS, DECAY_CONSTANT]
        let spec: [(&str, usize); 11] = [
            ("REMXS", g),
            ("NSF", g),
            ("DIFFCOEF", g),
            ("BETA_EFF", p),
            ("FISSXS", g),
            ("FISSE", g),
            ("CHI_T", g),
            ("CHI_P", g),
            ("CHI_D", g),
            ("GTRANSFXS", g * g),
            ("DECAY_CONSTANT", p),
        ];
        let mut tables: Vec<PropertyTable> = Vec::with_capacity(11);
        for (cname, o) in spec {
            let mut cols: Vec<Vec<f64>> = vec![Vec::with_capacity(l); o];
            if wanted.contains(&cname) {
                for t in &temps {
                    let key = format!("{}", *t as i64);
                    let ds = mat
                        .get(&key)
                        .ok_or_else(|| err(format!("no temperature block '{material_key}/{key}'")))?
                        .get(cname)
                        .and_then(serde_json::Value::as_array);
                    let empty = ds.is_none_or(Vec::is_empty);
                    if cname == "CHI_D" && empty {
                        for (k, c) in cols.iter_mut().enumerate() {
                            c.push(if k == 0 { 1.0 } else { 0.0 });
                        }
                        continue;
                    }
                    let ds = ds.ok_or_else(|| {
                        err(format!("Unable to open database {material_key}/{key}/{cname}"))
                    })?;
                    if ds.len() != o {
                        return Err(err(format!(
                            "the number of {material_key}/{key}/{cname} values does not match the num_groups/num_precursor_groups parameter. {}!={o}",
                            ds.len()
                        )));
                    }
                    for (k, c) in cols.iter_mut().enumerate() {
                        c.push(ds[k].as_f64().ok_or_else(|| {
                            err(format!("{material_key}/{key}/{cname}[{k}] is not a number"))
                        })?);
                    }
                }
            } else {
                for c in &mut cols {
                    c.resize(l, 0.0);
                }
            }
            tables.push(PropertyTable::from_columns(temps.clone(), cols));
        }
        Self::from_property_tables(tables, groups, precursor_groups, layout, interp)
    }

    /// Index into the flat `GTRANSFXS` row for the transfer `from -> to`.
    #[must_use]
    pub fn scatter_index(&self, from: usize, to: usize) -> usize {
        match self.layout {
            TableLayout::Serpent2 => from * self.groups + to,
            TableLayout::Legacy => from + to * self.groups,
        }
    }

    /// Delayed families `(beta_i, lambda_i)` at `t` (`BETA_EFF`,
    /// `DECAY_CONSTANT`).
    #[must_use]
    pub fn families_at(&self, t: f64) -> Vec<DelayedFamily> {
        (0..self.precursor_groups)
            .map(|i| DelayedFamily {
                beta: self.beta_eff.value(i, t, self.interp),
                lambda: self.decay.value(i, t, self.interp),
            })
            .collect()
    }

    /// Total delayed fraction `sum_i beta_i` at `t`, accumulated in
    /// Moltres' order (`NuclearMaterial::linearComputeQpProperties`).
    #[must_use]
    pub fn beta_total_at(&self, t: f64) -> f64 {
        let mut beta = 0.0;
        for i in 0..self.precursor_groups {
            beta += self.beta_eff.value(i, t, self.interp);
        }
        beta
    }

    /// Group constants at temperature `t` as an SI [`MsrMaterial`]
    /// (cm -> m, see the module docs). `d_sigma_removal_d_temp` carries
    /// Moltres' `d_remxs_d_temp` (slope of the interpolant at `t`, per
    /// metre). `chi_prompt` comes from `CHI_P` (`CHIP`) and `chi_delayed`
    /// from `CHI_D`.
    #[must_use]
    pub fn material_at(&self, name: &str, t: f64) -> MsrMaterial {
        material_from_samples(name, self.groups, self.layout, |c, k| {
            self.raw_sample(c, k, t).unwrap_or((f64::NAN, f64::NAN))
        })
    }

    /// Raw interpolated `CHI_T` at `t` (used when delayed neutrons are not
    /// tracked separately, Moltres `account_delayed = false`).
    #[must_use]
    pub fn chi_total_at(&self, t: f64) -> Vec<f64> {
        (0..self.groups)
            .map(|k| self.chi_t.value(k, t, self.interp))
            .collect()
    }

    /// Raw interpolated constant by Moltres name, for the kernel-level
    /// differential: `"REMXS"`, `"NSF"`, `"DIFFCOEF"`, `"BETA_EFF"`,
    /// `"FISSXS"`, `"FISSE"`, `"CHI_T"`, `"CHI_P"`, `"CHI_D"`,
    /// `"GTRANSFXS"`, `"DECAY_CONSTANT"`. Returns `(value, slope)` for
    /// column `k`, in the table's own (cm, MeV) units. `None` for an unknown
    /// name.
    #[must_use]
    pub fn raw_sample(&self, name: &str, k: usize, t: f64) -> Option<(f64, f64)> {
        let tab = match name {
            "REMXS" => &self.remxs,
            "NSF" => &self.nsf,
            "DIFFCOEF" => &self.diffcoef,
            "BETA_EFF" => &self.beta_eff,
            "FISSXS" => &self.fissxs,
            "FISSE" => &self.fisse,
            "CHI_T" => &self.chi_t,
            "CHI_P" => &self.chi_p,
            "CHI_D" => &self.chi_d,
            "GTRANSFXS" => &self.gtransfxs,
            "DECAY_CONSTANT" => &self.decay,
            _ => return None,
        };
        Some((tab.value(k, t, self.interp), tab.slope(k, t, self.interp)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_sample_holds_ends_and_interpolates() {
        let x = [800.0, 850.0, 900.0];
        let y = [1.0, 3.0, 4.0];
        assert_eq!(linear_sample(&x, &y, 700.0), 1.0);
        assert_eq!(linear_sample(&x, &y, 900.0), 4.0);
        assert_eq!(linear_sample(&x, &y, 1000.0), 4.0);
        assert!((linear_sample(&x, &y, 825.0) - 2.0).abs() < 1e-15);
        assert_eq!(linear_sample_derivative(&x, &y, 825.0), 2.0 / 50.0);
        assert_eq!(linear_sample_derivative(&x, &y, 900.0), 0.0);
        assert_eq!(linear_sample(&[5.0], &[7.0], 1.0), 7.0);
        assert!(linear_sample(&x, &y, f64::NAN).is_nan());
        assert!(linear_sample_derivative(&x, &y, f64::NAN).is_nan());
    }

    #[test]
    fn parse_rejects_bad_tables() {
        assert!(PropertyTable::parse("t", "1 2 3 4", 2).is_err());
        assert!(PropertyTable::parse("t", "2 1 \n 1 1", 1).is_err());
        assert!(PropertyTable::parse("t", "1 x", 1).is_err());
        assert!(PropertyTable::parse("t", "", 1).is_err());
        let t = PropertyTable::parse("t", "1 5\n2 7", 1).unwrap();
        assert_eq!(t.value(0, 1.5, InterpType::Linear), 6.0);
        assert_eq!(t.slope(0, 1.5, InterpType::Linear), 2.0);
        assert_eq!(t.value(0, 1.5, InterpType::None), 5.0);
        assert_eq!(t.slope(0, 1.5, InterpType::None), 0.0);
    }

    fn one_group_texts() -> [&'static str; 11] {
        [
            "900 0.02",   // REMXS
            "900 0.03",   // NSF
            "900 1.0",    // DIFFCOEF
            "900 0.0065", // BETA_EFF (1 family)
            "900 0.01",   // FISSXS
            "900 190.0",  // FISSE
            "900 1.0",    // CHI_T
            "900 1.0",    // CHI_P
            "900 1.0",    // CHI_D
            "900 0.5",    // GTRANSFXS
            "900 0.08",   // DECAY
        ]
    }

    #[test]
    fn material_at_converts_to_si_and_none_has_zero_slope() {
        let tabs = MoltresTables::from_texts(
            &one_group_texts(),
            Some("900 1.0"),
            1,
            1,
            TableLayout::Legacy,
            InterpType::None,
        )
        .unwrap();
        let m = tabs.material_at("fuel", 922.0);
        assert!((m.diffusion[0] - 0.01).abs() < 1e-18);
        assert!((m.sigma_removal[0] - 2.0).abs() < 1e-15);
        assert!((m.nu_sigma_f[0] - 3.0).abs() < 1e-15);
        assert_eq!(m.d_sigma_removal_d_temp[0], 0.0);
        assert_eq!(m.scattering[0][0], 0.0);
        assert!(m.validate(1).is_ok());
        assert_eq!(tabs.beta_total_at(900.0), 0.0065);
        assert_eq!(tabs.families_at(900.0)[0].lambda, 0.08);
        assert_eq!(tabs.chi_total_at(900.0), vec![1.0]);
        assert!(tabs.raw_sample("NOPE", 0, 900.0).is_none());
        assert!(tabs.raw_sample("FISSE", 0, 900.0).is_some());
    }

    #[test]
    fn none_rejects_multi_row_and_missing_chi_d_defaults_to_top_group() {
        let mut texts = one_group_texts();
        texts[0] = "900 0.02\n950 0.03";
        assert!(MoltresTables::from_texts(
            &texts,
            None,
            1,
            1,
            TableLayout::Serpent2,
            InterpType::None
        )
        .is_err());
        let ok = MoltresTables::from_texts(
            &one_group_texts(),
            None,
            1,
            1,
            TableLayout::Serpent2,
            InterpType::Linear,
        )
        .unwrap();
        assert_eq!(ok.material_at("f", 900.0).chi_delayed, vec![1.0]);
        assert_eq!(ok.scatter_index(0, 0), 0);
    }

    #[test]
    fn scatter_layouts_are_transposes_of_each_other() {
        let mk = |layout| {
            MoltresTables::from_texts(
                &[
                    "900 1 1",
                    "900 1 1",
                    "900 1 1",
                    "900 0.0065",
                    "900 1 1",
                    "900 1 1",
                    "900 1 0",
                    "900 1 0",
                    "900 1 0",
                    "900 0 10 20 0",
                    "900 0.08",
                ],
                None,
                2,
                1,
                layout,
                InterpType::None,
            )
            .unwrap()
        };
        let s2 = mk(TableLayout::Serpent2).material_at("a", 900.0);
        let lg = mk(TableLayout::Legacy).material_at("a", 900.0);
        // flat row [0, 10, 20, 0]: Serpent2 idx = from*2+to -> 0->1 = 10
        assert_eq!(s2.scattering[0][1], 1000.0);
        assert_eq!(s2.scattering[1][0], 2000.0);
        // Legacy idx = from + to*2 -> 0->1 = entry 2 = 20
        assert_eq!(lg.scattering[0][1], 2000.0);
        assert_eq!(lg.scattering[1][0], 1000.0);
    }

    #[test]
    fn spline_and_monotone_tables_prepare_cache_and_match_fallback() {
        let text = "800 1.0\n850 3.0\n900 4.0\n950 4.5";
        let raw = PropertyTable::parse("t", text, 1).unwrap();
        let ready_s = raw.clone().prepared("t", InterpType::Spline).unwrap();
        let ready_m = raw.clone().prepared("t", InterpType::MonotoneCubic).unwrap();
        for t in [790.0, 800.0, 833.3, 900.0, 940.0, 1000.0] {
            // the cached and the rebuilt-on-demand interpolators agree
            assert_eq!(
                ready_s.value(0, t, InterpType::Spline),
                raw.value(0, t, InterpType::Spline)
            );
            assert_eq!(
                ready_s.slope(0, t, InterpType::Spline),
                raw.slope(0, t, InterpType::Spline)
            );
            assert_eq!(
                ready_m.value(0, t, InterpType::MonotoneCubic),
                raw.value(0, t, InterpType::MonotoneCubic)
            );
            assert_eq!(
                ready_m.slope(0, t, InterpType::MonotoneCubic),
                raw.slope(0, t, InterpType::MonotoneCubic)
            );
        }
        // both pass through the nodes, unlike a clamped linear table outside
        assert!((ready_s.value(0, 850.0, InterpType::Spline) - 3.0).abs() < 1e-12);
        assert!((ready_m.value(0, 850.0, InterpType::MonotoneCubic) - 3.0).abs() < 1e-12);
        // end cells are extrapolated, not held (linear holds 4.5)
        assert!((ready_s.value(0, 1000.0, InterpType::Spline) - 4.5).abs() > 1e-3);
        assert_eq!(ready_s.value(0, 1000.0, InterpType::Linear), 4.5);
        // monotone cubic needs three rows; None / Linear need no cache
        let two = PropertyTable::parse("t", "800 1\n850 2", 1).unwrap();
        assert!(two.clone().prepared("t", InterpType::MonotoneCubic).is_err());
        assert!(two.clone().prepared("t", InterpType::Linear).is_ok());
        assert!(two.clone().prepared("t", InterpType::None).is_ok());
        // a one-row table cannot be splined: value falls back to NaN
        let one = PropertyTable::parse("t", "800 1", 1).unwrap();
        assert!(one.clone().prepared("t", InterpType::Spline).is_err());
        assert!(one.value(0, 800.0, InterpType::Spline).is_nan());
        assert!(one.slope(0, 800.0, InterpType::MonotoneCubic).is_nan());
    }

    #[test]
    fn from_texts_builds_spline_tables_and_rejects_two_row_monotone() {
        let mut texts = one_group_texts();
        texts[0] = "900 0.02\n950 0.03\n1000 0.05";
        let spl = MoltresTables::from_texts(
            &texts,
            Some("900 1.0"),
            1,
            1,
            TableLayout::Legacy,
            InterpType::Spline,
        );
        // the other ten tables have one row only, which cannot be splined
        assert!(spl.is_err());
        let mut rows3 = one_group_texts();
        for r in &mut rows3 {
            *r = "900 1\n950 1\n1000 1";
        }
        rows3[3] = "900 0.0065\n950 0.0065\n1000 0.0065";
        rows3[10] = "900 0.08\n950 0.08\n1000 0.08";
        rows3[0] = "900 0.02\n950 0.03\n1000 0.05";
        let ok = MoltresTables::from_texts(
            &rows3,
            Some("900 1\n950 1\n1000 1"),
            1,
            1,
            TableLayout::Legacy,
            InterpType::MonotoneCubic,
        )
        .unwrap();
        let (v, s) = ok.raw_sample("REMXS", 0, 950.0).unwrap();
        assert!((v - 0.03).abs() < 1e-15 && s > 0.0);
        let two = MoltresTables::from_texts(
            &one_group_texts().map(|_| "900 1\n950 1"),
            None,
            1,
            1,
            TableLayout::Legacy,
            InterpType::MonotoneCubic,
        );
        assert!(two.is_err());
    }
}
