// SPDX-License-Identifier: GPL-3.0-only
//! **OpenMC's non-geometry plotting functions, ported to emit standalone
//! matplotlib scripts.**
//!
//! Geometry plotting (`openmc --plot`, `src/plot.cpp`) is ported separately in
//! [`crate::geometry::plot`] and writes PNG directly. The functions here are
//! OpenMC's *Python* plotters, whose output is a matplotlib figure; the port
//! computes in Rust exactly the arrays upstream hands to matplotlib, and
//! writes a `.py` file that makes the same matplotlib calls on them (see
//! [`script`]). Run with the same matplotlib, that script draws a PNG
//! pixel-identical to OpenMC's — verified in
//! `verification_and_validation/python_plotting_parity/xs_and_tracks/`.
//!
//! | here | upstream (OpenMC `d7d3284a1`) |
//! |---|---|
//! | [`xs::plot_xs`], [`xs::calculate_cexs`] | `openmc.plot_xs`, `openmc.plotter.calculate_cexs` (`openmc/plotter.py:125-677`) |
//! | [`incident_neutron::IncidentNeutronData`] | the `IncidentNeutron` that `plot_xs` reads (ACE -> HDF5 -> back) |
//! | [`tracks::track_plot_script`], [`tracks::tracks_plot_script`] | `openmc.Track.plot`, `openmc.Tracks.plot` (`openmc/tracks.py:151-180`, `:255-275`) |
//!
//! # Surveyed and out of scope
//!
//! - **Multigroup `plot_xs` (`plot_CE=False`, `calculate_mgxs`)** — not
//!   ported; see [`xs`] for why (no MGXS representation with group edges here).
//! - **`openmc/data/multipole.py:360-383`** — the `matplotlib` block inside
//!   `_vectfit_xs` writes diagnostic PNGs (ACE vs vector-fitted cross section
//!   and relative error) *as a side effect of fitting windowed-multipole
//!   data*, when `path_out` is given. It is not a plotting API: nothing
//!   returns a figure, the data it draws exists only inside the fit, and it
//!   cannot be called on its own. It belongs with a port of the WMP
//!   generator (`njoy-outram-park-fork`'s `wmp` module), not here.
//! - Other `matplotlib` users in the OpenMC Python package are geometry or
//!   mesh plots (`Universe.plot`, `Model.plot`, `openmc.lib` plotting), which
//!   belong to [`crate::geometry::plot`].

pub mod endf_tables;
pub mod function1d;
pub mod incident_neutron;
pub mod numpy_ops;
pub mod script;
pub mod tracks;
pub mod xs;

pub use incident_neutron::{IncidentNeutronData, XsLibrary};
pub use script::FigureKwargs;
pub use tracks::{track_plot_script, tracks_plot_script, PlotTrack};
pub use xs::{
    calculate_cexs, plot_xs, EnergyAxisUnits, MaterialDensity, PlotMaterial, PlotTarget,
    PlotXsOptions, XsFigure, XsType,
};

/// Why a plot could not be made. The variants follow the Python exception
/// upstream raises in the same situation, plus [`PlotError::NotPorted`].
#[derive(Debug, Clone, PartialEq)]
pub enum PlotError {
    /// Upstream raises `TypeError`.
    TypeError(String),
    /// Upstream raises `ValueError`.
    ValueError(String),
    /// A branch of the upstream function this port does not carry; the
    /// message says why.
    NotPorted(String),
    /// The ACE table could not be read or is not what upstream expects.
    Ace(String),
}

impl std::fmt::Display for PlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeError(m) => write!(f, "TypeError: {m}"),
            Self::ValueError(m) => write!(f, "ValueError: {m}"),
            Self::NotPorted(m) => write!(f, "not ported: {m}"),
            Self::Ace(m) => write!(f, "ACE: {m}"),
        }
    }
}

impl std::error::Error for PlotError {}
