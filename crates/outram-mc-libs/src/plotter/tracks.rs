// SPDX-License-Identifier: GPL-3.0-only
//! **`openmc.Track.plot` and `openmc.Tracks.plot`**: 3-D polylines of particle
//! tracks.
//!
//! Ported from OpenMC `openmc/tracks.py` at commit `d7d3284a1` (0.16.1.dev25):
//! `Track.plot` (`:151-180`) and `Tracks.plot` (`:255-275`). OpenMC is
//! MIT-licensed; notice in `crates/outram-mc-libs/LICENSE.openmc`.
//!
//! Upstream draws one `ax.plot3D(x, y, z)` per *particle track* — a
//! `Track` is one source particle and holds the primary plus every secondary
//! it produced, each drawn as its own line (and so in its own colour from the
//! default cycle). [`PlotTrack`] mirrors that shape.
//!
//! # Where the tracks come from
//!
//! This crate records tracks already: [`crate::physics::track_output`]
//! (gh:#271), filled by `run_fixed_source_traced`. Each recorded
//! [`crate::physics::track_output::Track`] is one particle's history, so it
//! converts to a [`PlotTrack`] with a single particle track. The recorder does
//! not group secondaries under their source particle the way OpenMC's track
//! file does; a caller that wants that grouping builds the [`PlotTrack`]
//! directly.

use crate::geometry::position::Position;
use crate::physics::track_output::Track;

use super::script::{FigureKwargs, PyScript};

/// One source particle's tracks — the shape of `openmc.Track`: a list of
/// particle tracks (primary first), each a list of positions \[cm\].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlotTrack {
    /// `Track.particle_tracks[i].states['r']`, one entry per particle.
    pub particle_tracks: Vec<Vec<Position>>,
}

impl From<&Track> for PlotTrack {
    fn from(t: &Track) -> Self {
        Self {
            particle_tracks: vec![t.states.iter().map(|s| s.r).collect()],
        }
    }
}

fn axes_setup(s: &mut PyScript, fig: &FigureKwargs) {
    // `fig = plt.figure(); ax = plt.axes(projection='3d')` and the labels,
    // in upstream's order (`tracks.py:167-172`, `:266-270`).
    s.line(&format!("fig = plt.figure({})", fig.to_kwargs()));
    s.line("ax = plt.axes(projection='3d')");
    s.line("ax.set_xlabel('x [cm]')");
    s.line("ax.set_ylabel('y [cm]')");
    s.line("ax.set_zlabel('z [cm]')");
}

fn plot_one(s: &mut PyScript, track: &PlotTrack) {
    for states in &track.particle_tracks {
        let x: Vec<f64> = states.iter().map(|p| p.x).collect();
        let y: Vec<f64> = states.iter().map(|p| p.y).collect();
        let z: Vec<f64> = states.iter().map(|p| p.z).collect();
        let (ix, iy, iz) = (s.array(&x), s.array(&y), s.array(&z));
        s.line(&format!("ax.plot3D(a[{ix}], a[{iy}], a[{iz}])"));
    }
}

/// `Track.plot(axes=None)` as a standalone script.
pub fn track_plot_script(track: &PlotTrack, fig: &FigureKwargs, default_png: &str) -> String {
    let mut s = PyScript::new(
        "outram_mc_libs::plotter::tracks::track_plot_script",
        "openmc.Track.plot (openmc/tracks.py:151-180)",
    );
    axes_setup(&mut s, fig);
    plot_one(&mut s, track);
    s.finish(default_png)
}

/// `Tracks.plot()` as a standalone script.
pub fn tracks_plot_script(tracks: &[PlotTrack], fig: &FigureKwargs, default_png: &str) -> String {
    let mut s = PyScript::new(
        "outram_mc_libs::plotter::tracks::tracks_plot_script",
        "openmc.Tracks.plot (openmc/tracks.py:255-275)",
    );
    axes_setup(&mut s, fig);
    for t in tracks {
        plot_one(&mut s, t);
    }
    s.finish(default_png)
}
