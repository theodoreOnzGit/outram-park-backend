// SPDX-License-Identifier: GPL-3.0-only
//! **`openmc.plot_xs` and `openmc.plotter.calculate_cexs`**, continuous-energy.
//!
//! Ported from OpenMC `openmc/plotter.py` at commit `d7d3284a1`
//! (0.16.1.dev25), MIT-licensed (notice in
//! `crates/outram-mc-libs/LICENSE.openmc`):
//!
//! | here | upstream |
//! |---|---|
//! | [`PLOT_TYPES`], [`plot_types_mt`], [`PLOT_TYPES_LINEAR`] | `plotter.py:12-54` |
//! | `legend_label` | `_get_legend_label`, `:60-77` |
//! | `yaxis_label` | `_get_yaxis_label`, `:80-113` |
//! | `title` | `_get_title`, `:115-122` |
//! | [`plot_xs`] | `plot_xs`, `:125-290` |
//! | [`calculate_cexs`] | `calculate_cexs`, `:293-363` |
//! | `calculate_cexs_nuclide` | `_calculate_cexs_nuclide`, `:366-560` |
//! | `calculate_cexs_elem_mat` | `_calculate_cexs_elem_mat`, `:563-677` |
//! | [`PlotMaterial::get_nuclide_atom_densities`] | `Material.get_nuclide_atom_densities`, `openmc/material.py:1303-1385` |
//! | `expand_element` | `Element.expand`, `openmc/element.py:~60-326` (library-aware, no enrichment) |
//!
//! The output is an [`XsFigure`]: the arrays upstream hands to `ax.plot` and
//! the axis calls it makes, in order. [`XsFigure::to_python_script`] turns it
//! into a standalone matplotlib script that draws the same figure.
//!
//! # Upstream behaviours reproduced on purpose, not fixed
//!
//! These look like defects and were confirmed against the reference install
//! (see the V&V record); they are kept because the contract is "what OpenMC
//! draws":
//!
//! - **`'unity'` and `'slowing-down power'` do not work for a nuclide.** Both
//!   are built from the sentinel MTs `UNITY_MT = -1` / `XI_MT = -2`, which
//!   pass through `get_reaction_components`; that returns `[]` for an MT the
//!   nuclide does not have, so the sentinel never reaches the branch that
//!   would evaluate it. A nuclide's `'unity'` is identically 0 (and so is not
//!   plotted), and its `'slowing-down power'` is plain elastic, without `xi`.
//!   For a *material* `'unity'` is special-cased to 1 (`plotter.py:671-672`).
//! - **The linear-y branch is unreachable with separate type lists.**
//!   `all_types` is extended *before* `' / divisor'` is appended to the type
//!   names, so it never contains `'nu-fission / fission'` etc. and the
//!   `PLOT_TYPES_LINEAR` test fails; ratio plots come out log-log.
//! - A divisor of `'unity'` on a nuclide divides by that zero; `nan_to_num`
//!   then turns `x/0` into `1.797e308`.
//!
//! # Not ported, and why
//!
//! - **Multigroup (`plot_CE=False`, `calculate_mgxs`)**: it reads an
//!   `openmc.MGXSLibrary` (group edges, per-temperature `XSdata`, angle
//!   representations, delayed-group shapes). This crate's MG representation,
//!   `physics::physics_mg::MgxsLibrary`, carries no group edges or
//!   temperatures and none of `kappa-fission`, `inverse-velocity`, `beta`,
//!   `decay-rate`, `chi-prompt`/`chi-delayed`, and the workspace has an
//!   `mgxs.h5` *writer* but no reader. There is nothing it maps onto.
//!   Requesting it returns [`PlotError::NotPorted`].
//! - **S(a,b) (`sab_name`, a material's `_sab`)**: needs
//!   `ThermalScattering` (coherent elastic Bragg edges, `Regions1D`). The
//!   shared reference data holds no thermal-scattering ACE table both sides
//!   could read, so a port could not be verified here; it returns
//!   [`PlotError::NotPorted`] rather than a plot without the thermal data.
//! - **NCrystal** (`ncrystal_cfg`): an external C++ library.
//! - **Enrichment and weight-percent / mass-density compositions**: need
//!   `openmc.data.atomic_mass` (the AME2020 table), which is not in the
//!   workspace.
//! - **Several temperatures of one nuclide in one library**: an
//!   [`IncidentNeutronData`] holds one ACE table, so the nearest-temperature
//!   choice upstream makes is always that one table.

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use super::endf_tables::{
    dadz, gnds_name, is_element_name, matches_element, natural_isotopes, reaction_mt, zam,
};
use super::function1d::Tabulated1D;
use super::incident_neutron::{
    EmissionMode, Particle, PlotReaction, XsLibrary,
};
use super::numpy_ops::{interp, nan_to_num, pairwise_sum, union1d};
use super::script::{FigureKwargs, PyScript};
use super::PlotError;

/// `PLOT_TYPES` (`plotter.py:12-14`).
pub const PLOT_TYPES: [&str; 12] = [
    "total",
    "scatter",
    "elastic",
    "inelastic",
    "fission",
    "absorption",
    "capture",
    "nu-fission",
    "nu-scatter",
    "unity",
    "slowing-down power",
    "damage",
];

/// `PLOT_TYPES_LINEAR` (`plotter.py:49-50`).
pub const PLOT_TYPES_LINEAR: [&str; 4] = [
    "nu-fission / fission",
    "nu-scatter / scatter",
    "nu-fission / absorption",
    "fission / absorption",
];

/// `UNITY_MT` (`plotter.py:28`).
pub const UNITY_MT: i32 = -1;
/// `XI_MT` (`plotter.py:29`).
pub const XI_MT: i32 = -2;
/// `_MIN_E` \[eV\] (`plotter.py:53`).
pub const MIN_E: f64 = 1.0e-5;
/// `_MAX_E` \[eV\] (`plotter.py:54`).
pub const MAX_E: f64 = 20.0e6;

/// `_INELASTIC` (`plotter.py:32`): `SUM_RULES[3]` without 27.
fn inelastic() -> Vec<i32> {
    super::endf_tables::sum_rule(3)
        .expect("SUM_RULES[3]")
        .iter()
        .copied()
        .filter(|&m| m != 27)
        .collect()
}

/// `PLOT_TYPES_MT[name]` (`plotter.py:33-46`).
pub fn plot_types_mt(name: &str) -> Option<Vec<i32>> {
    Some(match name {
        "total" => super::endf_tables::sum_rule(1).expect("SUM_RULES[1]").to_vec(),
        "scatter" | "nu-scatter" => {
            let mut v = vec![2];
            v.extend(inelastic());
            v
        }
        "elastic" => vec![2],
        "inelastic" => inelastic(),
        "fission" | "nu-fission" => vec![18],
        "absorption" => vec![27],
        "capture" => vec![101],
        "unity" => vec![UNITY_MT],
        "slowing-down power" => vec![2, XI_MT],
        "damage" => vec![444],
        _ => return None,
    })
}

/// One entry of a `types` list: a name (`'total'`, `'(n,2n)'`, `'heating'`)
/// or an MT number — upstream accepts both in the same list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XsType {
    /// A string type.
    Named(String),
    /// An integer MT.
    Mt(i32),
}

impl From<&str> for XsType {
    fn from(s: &str) -> Self {
        Self::Named(s.to_string())
    }
}

impl From<i32> for XsType {
    fn from(m: i32) -> Self {
        Self::Mt(m)
    }
}

impl std::fmt::Display for XsType {
    /// Python's `f'{type}'`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Named(s) => f.write_str(s),
            Self::Mt(m) => write!(f, "{m}"),
        }
    }
}

impl XsType {
    fn is(&self, s: &str) -> bool {
        matches!(self, Self::Named(n) if n == s)
    }
}

/// `percent_type` of a material entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentType {
    /// Atom fraction / atom density (`'ao'`).
    Ao,
    /// Weight fraction (`'wo'`). Not supported by this port (needs atomic masses).
    Wo,
}

/// `Material.density_units` with its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaterialDensity {
    /// `'sum'`: the percents are atom densities \[atom/b-cm\] (upstream's default).
    Sum,
    /// `'macro'`.
    Macro(f64),
    /// `'atom/b-cm'`.
    AtomPerBarnCm(f64),
    /// `'atom/cm3'`.
    AtomPerCm3(f64),
    /// `'g/cm3'`. Not supported by this port (needs atomic masses).
    GramPerCm3(f64),
    /// `'kg/m3'`. Not supported by this port (needs atomic masses).
    KilogramPerM3(f64),
}

/// One `Material.add_nuclide(name, percent, percent_type)`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialNuclide {
    /// GNDS nuclide name.
    pub name: String,
    /// Amount.
    pub percent: f64,
    /// Kind of amount.
    pub percent_type: PercentType,
}

/// The parts of an `openmc.Material` that `plot_xs` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotMaterial {
    /// `Material.id`.
    pub id: i32,
    /// `Material.name` (`""` when unnamed).
    pub name: String,
    /// `Material.temperature`, which overrides `plot_xs`' temperature.
    pub temperature: Option<ThermodynamicTemperature>,
    /// Density and its units.
    pub density: MaterialDensity,
    /// Nuclides in insertion order.
    pub nuclides: Vec<MaterialNuclide>,
    /// `Material._sab` names. Must be empty (S(a,b) is not ported).
    pub sab: Vec<String>,
}

impl PlotMaterial {
    /// A named material with `'sum'` density and no nuclides yet.
    pub fn new(id: i32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            temperature: None,
            density: MaterialDensity::Sum,
            nuclides: Vec::new(),
            sab: Vec::new(),
        }
    }

    /// The plotting view of a transport [`crate::material::material::Material`]:
    /// each component's atom density \[atom/b-cm\] as an `'ao'` entry under
    /// `'sum'` density — what `openmc.Material.add_nuclide(n, d)` followed by
    /// `set_density('sum')` gives — named after `nuclides[idx].name`, with the
    /// material's temperature.
    pub fn from_transport(
        m: &crate::material::material::Material,
        nuclides: &[crate::material::nuclide::Nuclide],
    ) -> Self {
        let mut p = Self::new(m.id, &m.name);
        p.temperature = Some(ThermodynamicTemperature::new::<kelvin>(m.temperature));
        for c in &m.components {
            p = p.with_nuclide_ao(&nuclides[c.nuclide_idx].name, c.atom_density);
        }
        p
    }

    /// `add_nuclide(name, percent, 'ao')`.
    pub fn with_nuclide_ao(mut self, name: &str, percent: f64) -> Self {
        self.nuclides.push(MaterialNuclide {
            name: name.to_string(),
            percent,
            percent_type: PercentType::Ao,
        });
        self
    }

    /// `set_density(units, value)`.
    pub fn with_density(mut self, density: MaterialDensity) -> Self {
        self.density = density;
        self
    }

    /// `Material.get_nuclide_atom_densities()` (`material.py:1303-1385`),
    /// \[atom/b-cm\], in insertion order.
    pub fn get_nuclide_atom_densities(&self) -> Result<Vec<(String, f64)>, PlotError> {
        let (sum_density, mut density) = match self.density {
            MaterialDensity::Sum => (true, 0.0),
            MaterialDensity::Macro(d) => (false, d),
            MaterialDensity::GramPerCm3(d) => (false, -d),
            MaterialDensity::KilogramPerM3(d) => (false, -0.001 * d),
            MaterialDensity::AtomPerBarnCm(d) => (false, d),
            MaterialDensity::AtomPerCm3(d) => (false, 1.0e-24 * d),
        };
        let mut nd: Vec<f64> = self.nuclides.iter().map(|n| n.percent).collect();
        if sum_density {
            density = pairwise_sum(&nd);
        }
        let percent_in_atom = self.nuclides.iter().all(|n| n.percent_type == PercentType::Ao);
        let density_in_atom = density > 0.0;
        if !percent_in_atom {
            return Err(PlotError::NotPorted(
                "weight-percent compositions need openmc.data.atomic_mass (AME2020), \
                 which is not in the workspace"
                    .into(),
            ));
        }
        let sum_percent = pairwise_sum(&nd);
        for v in nd.iter_mut() {
            *v /= sum_percent;
        }
        if !density_in_atom {
            return Err(PlotError::NotPorted(
                "mass densities need Material.average_molar_mass, i.e. atomic masses \
                 (AME2020), which are not in the workspace"
                    .into(),
            ));
        }
        Ok(self
            .nuclides
            .iter()
            .zip(nd)
            .map(|(n, v)| (n.name.clone(), density * v))
            .collect())
    }
}

/// A key of `plot_xs`' `reactions` dict.
#[derive(Debug, Clone, PartialEq)]
pub enum PlotTarget {
    /// A nuclide (`"U235"`) or element (`"U"`) name.
    Nuclide(String),
    /// A material.
    Material(PlotMaterial),
}

impl From<&str> for PlotTarget {
    fn from(s: &str) -> Self {
        Self::Nuclide(s.to_string())
    }
}

/// `energy_axis_units`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnergyAxisUnits {
    /// `'eV'` (default).
    #[default]
    EV,
    /// `'keV'`.
    KeV,
    /// `'MeV'`.
    MeV,
}

impl EnergyAxisUnits {
    fn factor(self) -> f64 {
        match self {
            Self::EV => 1.0,
            Self::KeV => 1e-3,
            Self::MeV => 1e-6,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::EV => "eV",
            Self::KeV => "keV",
            Self::MeV => "MeV",
        }
    }
}

/// `plot_xs`' keyword arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotXsOptions {
    /// `temperature` (default 294 K).
    pub temperature: ThermodynamicTemperature,
    /// `sab_name`. Must be `None` (not ported).
    pub sab_name: Option<String>,
    /// `enrichment` \[wt%\]. Must be `None` (not ported).
    pub enrichment: Option<f64>,
    /// `plot_CE`. Must be `true` (multigroup not ported).
    pub plot_ce: bool,
    /// `energy_axis_units`.
    pub energy_axis_units: EnergyAxisUnits,
    /// `**kwargs` to `plt.subplots`.
    pub figure: FigureKwargs,
}

impl Default for PlotXsOptions {
    fn default() -> Self {
        Self {
            temperature: ThermodynamicTemperature::new::<kelvin>(294.0),
            sab_name: None,
            enrichment: None,
            plot_ce: true,
            energy_axis_units: EnergyAxisUnits::EV,
            figure: FigureKwargs::default(),
        }
    }
}

/// One `ax.plot(E, data[i, :], label=...)` call.
#[derive(Debug, Clone, PartialEq)]
pub struct XsLine {
    /// Energies, already scaled to the axis units.
    pub x: Vec<f64>,
    /// Values after `np.nan_to_num`.
    pub y: Vec<f64>,
    /// Legend label.
    pub label: String,
}

/// Everything `plot_xs` does to the axes, in call order.
#[derive(Debug, Clone, PartialEq)]
pub struct XsFigure {
    /// `plt.subplots(**kwargs)`.
    pub figure: FigureKwargs,
    /// The plotted lines, in call order (which fixes their colours).
    pub lines: Vec<XsLine>,
    /// `ax.set_yscale(...)`: `"log"`, or `"linear"` for the ratio types.
    pub yscale: &'static str,
    /// `ax.set_xlabel(...)`.
    pub xlabel: String,
    /// `ax.set_xlim(...)`.
    pub xlim: (f64, f64),
    /// `ax.set_ylabel(...)`.
    pub ylabel: String,
    /// `ax.set_title(...)`.
    pub title: String,
}

impl XsFigure {
    /// A standalone matplotlib script that draws this figure. The arrays are
    /// embedded (zlib + base64 of little-endian float64), so it needs only
    /// `numpy` and `matplotlib`. Run as `python3 script.py out.png`.
    pub fn to_python_script(&self, default_png: &str) -> String {
        let mut s = PyScript::new(
            "outram_mc_libs::plotter::xs::plot_xs",
            "openmc.plot_xs (openmc/plotter.py:125-290)",
        );
        s.line(&format!("fig, ax = plt.subplots({})", self.figure.to_kwargs()));
        for l in &self.lines {
            let ix = s.array(&l.x);
            let iy = s.array(&l.y);
            s.line(&format!(
                "ax.plot(a[{ix}], a[{iy}], label={})",
                super::script::py_str(&l.label)
            ));
        }
        s.line("ax.set_xscale('log')");
        s.line(&format!("ax.set_yscale('{}')", self.yscale));
        s.line(&format!("ax.set_xlabel({})", super::script::py_str(&self.xlabel)));
        s.line(&format!(
            "ax.set_xlim({}, {})",
            super::script::py_float(self.xlim.0),
            super::script::py_float(self.xlim.1)
        ));
        s.line(&format!("ax.set_ylabel({})", super::script::py_str(&self.ylabel)));
        s.line("ax.legend(loc='best')");
        s.line(&format!("ax.set_title({})", super::script::py_str(&self.title)));
        s.finish(default_png)
    }
}

/// `openmc.plot_xs` (`plotter.py:125-290`), continuous-energy.
///
/// `reactions` is upstream's dict as ordered pairs (dict order is plot
/// order, which fixes the line colours); `divisor_types` divides each type
/// line-for-line.
pub fn plot_xs(
    reactions: &[(PlotTarget, Vec<XsType>)],
    divisor_types: Option<&[XsType]>,
    library: &XsLibrary,
    opts: &PlotXsOptions,
) -> Result<XsFigure, PlotError> {
    if !opts.plot_ce {
        return Err(PlotError::NotPorted(
            "multigroup plotting (plot_CE=False / calculate_mgxs): no MGXS library \
             representation with group edges in this crate; see the module docs"
                .into(),
        ));
    }
    // A Python dict cannot hold the same key twice.
    for (i, (a, _)) in reactions.iter().enumerate() {
        for (b, _) in &reactions[..i] {
            if a == b {
                return Err(PlotError::ValueError("duplicate key in reactions".into()));
            }
        }
    }
    let divisor_types = divisor_types.filter(|d| !d.is_empty()); // `if divisor_types:`
    let temperature = opts.temperature.get::<kelvin>();
    let factor = opts.energy_axis_units.factor();
    let mut all_types: Vec<XsType> = Vec::new();
    let mut mutated: Vec<(&PlotTarget, Vec<XsType>)> = Vec::new();
    let mut lines = Vec::new();

    for (this, types) in reactions {
        let mut types = types.clone();
        all_types.extend(types.iter().cloned());
        let (mut e, mut data) =
            calculate_cexs(this, &types, temperature, opts.sab_name.as_deref(), library, opts.enrichment)?;
        if let Some(div) = divisor_types {
            if div.len() != types.len() {
                return Err(PlotError::ValueError(format!(
                    "Length of divisor types must be equal to {}",
                    types.len()
                )));
            }
            let (ediv, data_div) =
                calculate_cexs(this, div, temperature, opts.sab_name.as_deref(), library, opts.enrichment)?;
            let enum_ = e;
            e = union1d(&enum_, &ediv);
            let mut data_new = Vec::with_capacity(types.len());
            for line in 0..types.len() {
                let num = interp(&e, &enum_, &data[line]);
                let den = interp(&e, &ediv, &data_div[line]);
                data_new.push(num.iter().zip(den.iter()).map(|(a, b)| a / b).collect::<Vec<_>>());
                if !div[line].is("unity") {
                    types[line] = match (&types[line], &div[line]) {
                        (XsType::Named(a), XsType::Named(b)) => XsType::Named(format!("{a} / {b}")),
                        _ => {
                            return Err(PlotError::TypeError(
                                "unsupported operand type(s) for +: 'int' and 'str' \
                                 (an MT number cannot take a divisor suffix)"
                                    .into(),
                            ))
                        }
                    };
                }
            }
            data = data_new;
        }
        for v in e.iter_mut() {
            *v *= factor;
        }
        for (i, row) in data.iter_mut().enumerate() {
            for v in row.iter_mut() {
                *v = nan_to_num(*v);
            }
            if pairwise_sum(row) > 0.0 {
                lines.push(XsLine {
                    x: e.clone(),
                    y: row.clone(),
                    label: legend_label(this, &types[i])?,
                });
            }
        }
        mutated.push((this, types));
    }

    let linear = all_types
        .iter()
        .all(|t| matches!(t, XsType::Named(n) if PLOT_TYPES_LINEAR.contains(&n.as_str())));
    let units = opts.energy_axis_units;
    Ok(XsFigure {
        figure: opts.figure.clone(),
        lines,
        yscale: if linear { "linear" } else { "log" },
        xlabel: format!("Energy [{}]", units.label()),
        xlim: (MIN_E * factor, MAX_E * factor),
        ylabel: yaxis_label(&mutated, divisor_types.is_some())?,
        title: title(reactions),
    })
}

/// `_get_legend_label` (`plotter.py:60-77`).
fn legend_label(this: &PlotTarget, ty: &XsType) -> Result<String, PlotError> {
    match this {
        PlotTarget::Nuclide(name) => {
            if let XsType::Named(t) = ty {
                if let Some((da, dz)) = dadz(t) {
                    if is_element_name(name) {
                        return Ok(format!("{name} {t}"));
                    }
                    let (z, a, m) = zam(name).ok_or_else(|| {
                        PlotError::ValueError(format!(
                            "'{name}' does not appear to be a nuclide name in GNDS format"
                        ))
                    })?;
                    let product = gnds_name((z as i32 + dz) as u32, (a as i32 + da) as u32, m)
                        .ok_or_else(|| PlotError::ValueError(format!("no product for {name} {t}")))?;
                    return Ok(format!("{name} {t} {product}"));
                }
            }
            Ok(format!("{name} {ty}"))
        }
        PlotTarget::Material(m) if m.name.is_empty() => Ok(format!("Material {} {ty}", m.id)),
        PlotTarget::Material(m) => Ok(format!("{} {ty}", m.name)),
    }
}

/// `_get_yaxis_label` (`plotter.py:80-113`), on the type lists *after* the
/// divisor suffixes were appended (upstream mutates them in place).
fn yaxis_label(reactions: &[(&PlotTarget, Vec<XsType>)], divisor: bool) -> Result<String, PlotError> {
    const HEAT: [&str; 3] = ["heating", "heating-local", "damage-energy"];
    let is_heat = |t: &XsType| matches!(t, XsType::Named(n) if HEAT.contains(&n.as_str()));
    let stem = if reactions.iter().all(|(_, v)| v.iter().all(is_heat)) {
        "Heating"
    } else if reactions.iter().all(|(k, _)| matches!(k, PlotTarget::Nuclide(_))) {
        if reactions.iter().any(|(_, v)| v.iter().any(is_heat)) {
            return Err(PlotError::TypeError(
                "Mixture of heating and Microscopic reactions. Invalid type for plotting".into(),
            ));
        }
        "Microscopic"
    } else if reactions.iter().all(|(k, _)| matches!(k, PlotTarget::Material(_))) {
        "Macroscopic"
    } else {
        return Err(PlotError::TypeError(
            "Mixture of openmc.Material and elements/nuclides. Invalid type for plotting".into(),
        ));
    };
    let (mid, units) = if divisor {
        ("Data", "")
    } else {
        (
            "Cross Section",
            match stem {
                "Macroscopic" => "[1/cm]",
                "Microscopic" => "[b]",
                _ => "[eV-barn]",
            },
        )
    };
    Ok(format!("{stem} {mid} {units}"))
}

/// `_get_title` (`plotter.py:115-122`).
fn title(reactions: &[(PlotTarget, Vec<XsType>)]) -> String {
    if reactions.len() == 1 {
        let name = match &reactions[0].0 {
            PlotTarget::Nuclide(n) => n.clone(),
            PlotTarget::Material(m) => m.name.clone(),
        };
        format!("Cross Section Plot For {name}")
    } else {
        "Cross Section Plot".to_string()
    }
}

/// `calculate_cexs` (`plotter.py:293-363`): the energy grid \[eV\] and one row
/// per type.
pub fn calculate_cexs(
    this: &PlotTarget,
    types: &[XsType],
    temperature_k: f64,
    sab_name: Option<&str>,
    library: &XsLibrary,
    enrichment: Option<f64>,
) -> Result<(Vec<f64>, Vec<Vec<f64>>), PlotError> {
    match this {
        PlotTarget::Nuclide(name) if is_element_name(name) => {
            calculate_cexs_elem_mat(this, types, temperature_k, library, sab_name, enrichment)
        }
        PlotTarget::Nuclide(name) => {
            let (e, funcs) = calculate_cexs_nuclide(name, types, temperature_k, sab_name, library)?;
            let data = funcs.iter().map(|f| f.eval(&e)).collect();
            Ok((e, data))
        }
        PlotTarget::Material(_) => {
            calculate_cexs_elem_mat(this, types, temperature_k, library, None, None)
        }
    }
}

/// A term of a type's `Combination` (`plotter.py:497-557`).
#[derive(Debug, Clone)]
enum Term<'a> {
    /// `nuc[mt].xs[nucT]`.
    Xs(&'a PlotReaction),
    /// `Combination([xs, total_yield], [np.multiply])`.
    XsTimesYield(&'a PlotReaction, &'a super::function1d::Function1D),
    /// `Combination([func, xs], [np.multiply])` with `func` the nested sum of
    /// the non-total neutron yields, first product innermost.
    YieldSumTimesXs(Vec<&'a super::function1d::Function1D>, &'a PlotReaction),
    /// `lambda x: c`.
    Const(f64),
}

impl Term<'_> {
    fn eval(&self, e: &[f64]) -> Vec<f64> {
        match self {
            Term::Xs(r) => r.xs.eval(e),
            Term::XsTimesYield(r, y) => {
                let a = r.xs.eval(e);
                let b = y.eval(e);
                a.iter().zip(b.iter()).map(|(p, q)| p * q).collect()
            }
            Term::YieldSumTimesXs(ys, r) => {
                let mut acc = ys[0].eval(e);
                for y in &ys[1..] {
                    let v = y.eval(e);
                    // Combination([prod.yield_, func], [np.add])
                    acc = v.iter().zip(acc.iter()).map(|(p, q)| p + q).collect();
                }
                let x = r.xs.eval(e);
                acc.iter().zip(x.iter()).map(|(p, q)| p * q).collect()
            }
            Term::Const(c) => vec![*c; e.len()],
        }
    }
}

/// `Combination(funcs, ops)`.
#[derive(Debug, Clone)]
struct Combination<'a> {
    funcs: Vec<Term<'a>>,
    /// `true` = `np.multiply`, `false` = `np.add`.
    ops_mul: Vec<bool>,
}

impl Combination<'_> {
    fn eval(&self, e: &[f64]) -> Vec<f64> {
        let mut ans = self.funcs[0].eval(e);
        for (i, &mul) in self.ops_mul.iter().enumerate() {
            let v = self.funcs[i + 1].eval(e);
            for (a, b) in ans.iter_mut().zip(v.iter()) {
                *a = if mul { *a * b } else { *a + b };
            }
        }
        ans
    }
}

/// `_calculate_cexs_nuclide` (`plotter.py:366-560`).
fn calculate_cexs_nuclide<'a>(
    this: &str,
    types: &[XsType],
    _temperature_k: f64,
    sab_name: Option<&str>,
    library: &'a XsLibrary,
) -> Result<(Vec<f64>, Vec<Combination<'a>>), PlotError> {
    let nuc = library
        .get_by_material(this)
        .ok_or_else(|| PlotError::ValueError(format!("{this} not in library")))?;
    // Nearest temperature: an IncidentNeutronData holds exactly one, so the
    // `strT in nuc.temperatures` / argmin choice (`:393-399`) is that one.
    if sab_name.is_some() {
        return Err(PlotError::NotPorted(
            "S(a,b) cross sections (sab_name): no thermal-scattering ACE in the shared \
             reference data to verify a port against"
                .into(),
        ));
    }
    let energy_grid = nuc.energy.clone();

    let mut mts: Vec<Vec<i32>> = Vec::new();
    let mut ops: Vec<Vec<bool>> = Vec::new();
    let mut yields: Vec<bool> = Vec::new();
    for line in types {
        let named_plot_type = match line {
            XsType::Named(s) if PLOT_TYPES.contains(&s.as_str()) => Some(s.as_str()),
            _ => None,
        };
        if let Some(s) = named_plot_type {
            let tmp: Vec<i32> = plot_types_mt(s)
                .expect("plot type")
                .iter()
                .flat_map(|&mti| nuc.get_reaction_components(mti))
                .collect();
            yields.push(s.starts_with("nu"));
            if tmp.contains(&XI_MT) {
                let mut o = vec![false; tmp.len().saturating_sub(2)];
                o.push(true);
                ops.push(o);
            } else {
                ops.push(vec![false; tmp.len().saturating_sub(1)]);
            }
            mts.push(tmp);
            continue;
        }
        let mt = match line {
            XsType::Named(s) => reaction_mt(s)
                .ok_or_else(|| PlotError::TypeError(format!("Invalid type {s:?}")))?,
            XsType::Mt(m) => *m,
        };
        if mt <= 0 {
            return Err(PlotError::ValueError(format!(
                "Unable to set \"MT in types\" to \"{mt}\" since it is less than or equal to \"0\""
            )));
        }
        let tmp = nuc.get_reaction_components(mt);
        ops.push(vec![false; tmp.len().saturating_sub(1)]);
        yields.push(false);
        mts.push(tmp);
    }

    let awr = nuc.atomic_weight_ratio;
    let mut xs = Vec::with_capacity(mts.len());
    for (i, mt_set) in mts.iter().enumerate() {
        let mut funcs: Vec<Term> = Vec::new();
        for &mt in mt_set {
            if mt == 2 {
                funcs.push(Term::Xs(&nuc.reactions[&2]));
            } else if let Some(r) = nuc.reactions.get(&mt) {
                if yields[i] {
                    let neutron = |p: &&super::incident_neutron::Product| p.particle == Particle::Neutron;
                    let all = || r.products.iter().chain(r.derived_products.iter()).filter(neutron);
                    if let Some(p) = all().find(|p| p.emission_mode == EmissionMode::Total) {
                        funcs.push(Term::XsTimesYield(r, p.yield_.as_ref().expect("neutron yield")));
                    } else {
                        let ys: Vec<_> = all()
                            .filter(|p| p.emission_mode != EmissionMode::Total)
                            .map(|p| p.yield_.as_ref().expect("neutron yield"))
                            .collect();
                        if ys.is_empty() {
                            funcs.push(Term::Xs(r));
                        } else {
                            funcs.push(Term::YieldSumTimesXs(ys, r));
                        }
                    }
                } else {
                    funcs.push(Term::Xs(r));
                }
            } else if mt == UNITY_MT {
                funcs.push(Term::Const(1.0));
            } else if mt == XI_MT {
                let alpha = ((awr - 1.0) / (awr + 1.0)).powi(2);
                let xi = 1.0 + alpha * alpha.ln() / (1.0 - alpha);
                funcs.push(Term::Const(xi));
            } else {
                funcs.push(Term::Const(0.0));
            }
        }
        if funcs.is_empty() {
            funcs.push(Term::Const(0.0));
        }
        xs.push(Combination {
            funcs,
            ops_mul: ops[i].clone(),
        });
    }
    Ok((energy_grid, xs))
}

/// `Element.expand(1., 'ao', enrichment, cross_sections)`
/// (`openmc/element.py`), for the library-aware branch `plot_xs` always takes.
fn expand_element(
    element: &str,
    library: &XsLibrary,
    enrichment: Option<f64>,
) -> Result<Vec<(String, f64)>, PlotError> {
    if enrichment.is_some() {
        return Err(PlotError::NotPorted(
            "element enrichment needs openmc.data.atomic_mass (AME2020), not in the workspace"
                .into(),
        ));
    }
    let natural = natural_isotopes(element);
    let library_nuclides: Vec<&str> = library
        .names()
        .into_iter()
        .filter(|n| matches_element(element, n))
        .collect();
    let key = |n: &&str| zam(n).unwrap_or((0, 0, 0));
    let mut mutual: Vec<&str> = natural
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| library_nuclides.contains(n))
        .collect();
    let mut absent: Vec<&str> = natural
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !mutual.contains(n))
        .collect();
    mutual.sort_by_key(key);
    absent.sort_by_key(key);
    let abundance = |n: &str| natural.iter().find(|(m, _)| *m == n).map(|x| x.1).expect("natural");
    let mut ab: Vec<(String, f64)> = Vec::new();
    let natural_name = format!("{element}0");
    if absent.is_empty() {
        for n in &mutual {
            ab.push((n.to_string(), abundance(n)));
        }
    } else if library_nuclides.contains(&natural_name.as_str()) {
        ab.push((natural_name, 1.0));
    } else if mutual.is_empty() {
        return Err(PlotError::ValueError(format!(
            "Unable to expand element {element} because the cross section library provided \
             does not contain any of the natural isotopes for that element."
        )));
    } else {
        for n in &mutual {
            ab.push((n.to_string(), abundance(n)));
        }
        for n in &absent {
            let target = match *n {
                "O17" | "O18" if mutual.contains(&"O16") => "O16",
                "Ta180_m1" if mutual.contains(&"Ta181") => "Ta181",
                "W180" if mutual.contains(&"W182") => "W182",
                _ => {
                    return Err(PlotError::ValueError(format!(
                        "Unsure how to partition natural abundance of isotope {n} into other \
                         natural isotopes of this element that are present in the cross \
                         section library provided."
                    )))
                }
            };
            let add = abundance(n);
            ab.iter_mut().find(|(m, _)| m == target).expect("target").1 += add;
        }
    }
    // `isotopes.append((nuclide, percent * abundance, percent_type))`, percent = 1.
    Ok(ab.into_iter().map(|(n, a)| (n, 1.0 * a)).collect())
}

/// `_calculate_cexs_elem_mat` (`plotter.py:563-677`).
fn calculate_cexs_elem_mat(
    this: &PlotTarget,
    types: &[XsType],
    temperature_k: f64,
    library: &XsLibrary,
    sab_name: Option<&str>,
    enrichment: Option<f64>,
) -> Result<(Vec<f64>, Vec<Vec<f64>>), PlotError> {
    let (t, fractions) = match this {
        PlotTarget::Material(m) => {
            if !m.sab.is_empty() {
                return Err(PlotError::NotPorted(
                    "S(a,b) on a material: no thermal-scattering data to verify a port against"
                        .into(),
                ));
            }
            let t = m
                .temperature
                .map(|t| t.get::<kelvin>())
                .unwrap_or(temperature_k);
            (t, m.get_nuclide_atom_densities()?)
        }
        PlotTarget::Nuclide(el) => {
            if sab_name.is_some() {
                return Err(PlotError::NotPorted("S(a,b) (sab_name) is not ported".into()));
            }
            (temperature_k, expand_element(el, library, enrichment)?)
        }
    };
    if fractions.is_empty() {
        return Err(PlotError::ValueError(
            "no nuclides to combine (upstream fails indexing E[0])".into(),
        ));
    }
    let mut grids = Vec::with_capacity(fractions.len());
    let mut tabs: Vec<Vec<Tabulated1D>> = Vec::with_capacity(fractions.len());
    for (name, _) in &fractions {
        let (e, rows) = calculate_cexs(
            &PlotTarget::Nuclide(name.clone()),
            types,
            t,
            None,
            library,
            None,
        )?;
        tabs.push(
            rows.into_iter()
                .map(|r| Tabulated1D::lin_lin(e.clone(), r))
                .collect(),
        );
        grids.push(e);
    }
    let mut energy_grid = grids[0].clone();
    for g in &grids[1..] {
        energy_grid = union1d(&energy_grid, g);
    }
    let mut data = vec![vec![0.0; energy_grid.len()]; types.len()];
    for (line, row) in data.iter_mut().enumerate() {
        if types[line].is("unity") {
            row.iter_mut().for_each(|v| *v = 1.0);
        } else {
            for ((_, frac), tab) in fractions.iter().zip(tabs.iter()) {
                let v = tab[line].eval(&energy_grid);
                for (d, x) in row.iter_mut().zip(v.iter()) {
                    *d += frac * x;
                }
            }
        }
    }
    Ok((energy_grid, data))
}
