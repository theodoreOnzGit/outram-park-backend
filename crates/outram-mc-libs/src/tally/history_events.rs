// SPDX-License-Identifier: GPL-3.0

//! **History-event tally driver** — supplies the event-history fields of
//! [`FilterEvent`] that the transport loop does not (GitHub #261).
//!
//! # Why this exists
//!
//! [`FilterEvent`] carries `cell_from`, `cell_born`, `material_from`,
//! `surface_mu`, `position_born`, `n_collision` and `weight`, and the filters
//! [`CellFromFilter`](super::filter_extra::CellFromFilter),
//! [`CellBornFilter`](super::filter_extra::CellBornFilter),
//! [`MaterialFromFilter`](super::filter_extra::MaterialFromFilter),
//! [`MuSurfaceFilter`](super::filter_extra::MuSurfaceFilter),
//! [`MeshBornFilter`](super::filter_extra::MeshBornFilter),
//! [`MeshSurfaceFilter`](super::filter_extra::MeshSurfaceFilter),
//! [`CollisionFilter`](super::filter_extra::CollisionFilter) and
//! [`WeightFilter`](super::filter_extra::WeightFilter) read them. Measured by
//! `grep` on 2026-11-18: **no code outside `tally/filter*.rs` sets any of those
//! fields**; `scoring.rs` builds its events with `..Default::default()`. A tally
//! carrying one of those filters on a real run therefore scores nothing, or bins
//! every event at the default value.
//!
//! The transport function `transport_history_vr` is kovan-reviewed and is not
//! edited. This module sits **beside** it: it consumes the
//! [`TraceEvent`] stream the reviewed function already emits through its
//! observer (`trace_csg_history`), and rebuilds the per-event [`FilterEvent`]s
//! from it. It draws no random number and changes no transport result.
//!
//! # Upstream semantics mirrored (OpenMC 0.15.3, `27e38e89`)
//!
//! - `cell_born`: `particle.cpp:188`, the lowest-level cell of the first locate.
//! - `cell_from`, `material_from`: the cell / material the particle was in
//!   **before the surface crossing** (`particle.cpp:290-295`, `geometry.cpp:176`).
//! - `surface_mu`: `filter_musurface.cpp:13-30`. The unit normal is flipped when
//!   the crossing token is negative, which (`geometry.cpp:426-434`) happens
//!   exactly when `u . n < 0`, so the cosine is `|u . n|`.
//! - the sign of a `current` score through a surface filter:
//!   `filter_surface.cpp:55-60`, `-1` when `p.surface() < 0`, else `+1`. This
//!   crate's [`SurfaceFilter`](super::filter::SurfaceFilter) carries no sign, so
//!   the sign is applied here, in the scorer, and is stated in the V&V record.
//! - `n_collision`: collisions completed **before** the segment being scored
//!   (the track-length estimator scores in `event_advance`, before
//!   `event_collide` increments the count).
//! - mesh-surface currents are scored per flight segment from its start to its
//!   end (`filter_meshsurface.cpp`), so each [`HistoryEventKind::Segment`]
//!   carries both end points.
//!
//! # Limits, stated
//!
//! Scores supported: [`ScoreType::Flux`] on segments (`w * d`) and
//! [`ScoreType::Current`] on crossings and, for a mesh-surface tally, on
//! segments. A tally with a mesh-surface filter must carry no other filter
//! (one event can match several faces; combining would need a product over
//! multi-match filters). Time is not reconstructed (`time = 0`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::geometry::cell::{RegionToken, SurfaceToken};
use crate::geometry::geometry::Coord;
use crate::geometry::distribcell::DistribcellOffsets;
use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};
use crate::physics::track_output::TrackEvent;
use crate::physics::tracking_trace::TraceEvent;
use crate::tally::filter::{FilterEvent, FilterKind};
use crate::tally::mesh::{MeshKind, RegularMesh, RegularMeshExt};
use crate::tally::tally::{ScoreType, Tally};

/// What kind of tallying event a [`HistoryEvent`] is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HistoryEventKind {
    /// One flight segment (a track-length event) of the given length \[cm\].
    Segment {
        /// Segment length \[cm\].
        distance: f64,
    },
    /// A surface crossing. `sign` is OpenMC's `p.surface() < 0 ? -1 : +1`.
    Crossing {
        /// `+1.0` when the particle moves along the outward normal, else `-1.0`.
        sign: f64,
    },
    /// A real collision. `scatter` is set once the outcome is known to be a
    /// scatter (then `ev.mu` is the lab-frame cosine and `ev.energy_out` the
    /// outgoing energy).
    Collision {
        /// The collision ended in a scattering event.
        scatter: bool,
    },
}

/// One tallying event with the filter-facing state it was scored in.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEvent {
    /// Segment, crossing or collision.
    pub kind: HistoryEventKind,
    /// The state every filter reads.
    pub ev: FilterEvent,
    /// For a segment: the point the flight ended at, as OpenMC's
    /// `score_surface_tally` sees it for a mesh-surface tally (`p.r()`), with a
    /// vacuum leak moved on by `TINY_BIT` along `u` (`particle.cpp:638-642`).
    /// [`FilterEvent::position`] is the segment *midpoint* and cannot serve.
    /// For other events, equal to `ev.position`.
    pub track_end: Position,
}

/// OpenMC's `TINY_BIT` (`constants.h`): how far a particle is pushed past a
/// vacuum boundary so a mesh face lying on that boundary still counts.
pub const TINY_BIT: f64 = 1.0e-8;

/// The transport's per-crossing nudge (`transport_csg.rs:2051`), undone here so
/// a flight that begins at a boundary is reconstructed from the boundary.
const TRANSPORT_NUDGE: f64 = 1.0e-9;

/// The sign OpenMC gives a crossing of `surface` (`p.surface() < 0 ? -1 : +1`).
///
/// `levels` is the coordinate stack of the cell the particle leaves, `d` the
/// distance to the surface and `surface` its **global** index. The surface
/// lives in the frame of the deepest level whose cell region references it, so
/// the crossing is evaluated there (`geometry.cpp:426-434`: the sign is that of
/// `u . n` with `n` taken at the hit point in that level's local frame). Using
/// the global frame instead gives the wrong sign for any surface inside a
/// translated universe or a lattice tile; the first version of this function
/// did, and the Experiment A cell-from tally caught it.
pub fn crossing_sign(geom: &Geometry, levels: &[Coord], surface: usize, d: f64) -> f64 {
    let level = levels.iter().rev().find(|lv| {
        geom.cells[lv.cell].region.iter().any(|t| {
            matches!(t, RegionToken::HalfSpace { surface_idx, .. } if *surface_idx == surface)
        })
    });
    let Some(lv) = level else { return 1.0 };
    let hit = advance(lv.r, lv.u, d);
    let n = geom.surfaces[surface].normal(hit);
    let dot = lv.u.u * n.u + lv.u.v * n.v + lv.u.w * n.w;
    if dot < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// `mu` of `filter_musurface.cpp:13-30`: the unit normal **at the global
/// position** `p.r()` (upstream passes the global position, whatever frame the
/// surface is defined in, so for a surface inside a lattice tile this is the
/// normal of the surface evaluated at the wrong point; reproduced, not fixed),
/// flipped when `sign < 0`, dotted with the **global** direction, clamped.
pub fn crossing_mu(
    geom: &Geometry,
    surface: usize,
    p_global: Position,
    u_global: Direction,
    sign: f64,
) -> f64 {
    let n = geom.surfaces[surface].normal(p_global);
    let norm = (n.u * n.u + n.v * n.v + n.w * n.w).sqrt();
    let mut dot = (u_global.u * n.u + u_global.v * n.v + u_global.w * n.w) / norm;
    if sign < 0.0 {
        dot = -dot;
    }
    dot.clamp(-1.0, 1.0)
}

fn advance(r: Position, u: Direction, d: f64) -> Position {
    Position { x: r.x + d * u.u, y: r.y + d * u.v, z: r.z + d * u.w }
}

// ────────────────────────────────────────────────────────────────────────────
// Live path (gh:#261, 2026-11-18, maintainer-confirmed): the same event-history
// fields, kept by the real transport loop and scored while it runs.
// ────────────────────────────────────────────────────────────────────────────

/// Per-particle event-history state the real transport keeps so that the
/// history-field filters have something to read (gh:#261).
///
/// One is made per particle popped from the history stack, because OpenMC
/// resets these on every `from_source` (`particle.cpp:115-121,135`: `cell_born`,
/// `n_collision`, `r_born`), which also covers (n,2n) secondaries and weight-
/// window daughters, which come back through the bank. Updating it draws no
/// random number and takes no decision, so a run with it is the run without it
/// bit for bit.
///
/// `material_last` follows OpenMC: it is set to the previous material by every
/// cell search (`geometry.cpp:176`) and cleared by a collision
/// (`particle.cpp:392`); the relocation the CSG loop does after a collision is
/// **not** a cell search upstream, so it leaves `material_last` cleared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventHistory {
    /// Lowest-level cell of the first locate (`particle.cpp:188`).
    pub cell_born: Option<usize>,
    /// Birth position (`particle.cpp:135`).
    pub position_born: Position,
    /// Collisions completed so far by this particle (`physics.cpp:46`).
    pub n_collision: u32,
    /// Material of the cell the particle is in (`None` in a void).
    pub material: Option<usize>,
    /// OpenMC's `material_last`.
    pub material_last: Option<usize>,
    /// The current flight began at a surface or lattice crossing, whose 1e-9 cm
    /// nudge [`score_mesh_surface_segment`] undoes.
    pub after_boundary: bool,
    after_collision: bool,
}

impl Default for EventHistory {
    /// The state of no history at all: what a caller that does not track one
    /// gets, equal to the fields `FilterEvent::default()` carries.
    fn default() -> Self {
        EventHistory {
            cell_born: None,
            position_born: Position::ZERO,
            n_collision: 0,
            material: None,
            material_last: None,
            after_boundary: false,
            after_collision: false,
        }
    }
}

impl EventHistory {
    /// A particle born at `r`.
    pub fn at_birth(r: Position) -> Self {
        EventHistory { position_born: r, ..Default::default() }
    }

    /// The loop located the particle in `cell` with `material`.
    pub fn located(&mut self, cell: usize, material: Option<usize>) {
        if !self.after_collision {
            self.material_last = self.material;
        }
        self.after_collision = false;
        self.material = material;
        if self.cell_born.is_none() {
            self.cell_born = Some(cell);
        }
    }

    /// A real collision was completed.
    pub fn collided(&mut self) {
        self.n_collision += 1;
        self.material_last = None;
        self.after_collision = true;
        self.after_boundary = false;
    }

    /// The flight ended at a surface or lattice boundary.
    pub fn crossed_boundary(&mut self) {
        self.after_boundary = true;
    }
}

/// The start and end of a flight as `score_surface_tally` sees them for a
/// mesh-surface tally: the start is moved back over the transport's crossing
/// nudge when the flight began at a boundary, and the end of a flight that
/// leaves through a vacuum surface is pushed on by [`TINY_BIT`]
/// (`particle.cpp:638-642`).
pub fn flight_track_ends(
    r: Position,
    u: Direction,
    d: f64,
    after_boundary: bool,
    leaks: bool,
) -> (Position, Position) {
    let end = advance(r, u, d);
    let track_end = if leaks { advance(end, u, TINY_BIT) } else { end };
    let start = if after_boundary { advance(r, u, -TRANSPORT_NUDGE) } else { r };
    (start, track_end)
}

/// Whether `tally` is one [`score_crossing`] scores: a `Current` score on a
/// `SurfaceFilter`, without a mesh-surface filter (that one scores per flight).
pub fn scores_crossings(tally: &Tally) -> bool {
    tally.scores.iter().any(|s| matches!(s, ScoreType::Current))
        && tally.filters.iter().any(|f| matches!(f, FilterKind::Surface(_)))
        && !tally.filters.iter().any(|f| matches!(f, FilterKind::MeshSurface(_)))
}

/// Score one surface crossing into `batch` (`tally.bins.len()` entries) for a
/// tally [`scores_crossings`] accepts; any other tally is left alone.
///
/// The event is built exactly as [`history_events`] builds a crossing:
/// `r_flight`, `u` and `d` are the flight that ended at `surface`, `levels` the
/// coordinate stack it flew in, `leaked` whether the crossing killed the
/// particle (then `material_from` is OpenMC's stale `material_last`). The
/// weight is the particle's, signed by [`crossing_sign`].
#[allow(clippy::too_many_arguments)]
pub fn score_crossing(
    batch: &mut [f64],
    tally: &Tally,
    geom: &Geometry,
    levels: &[Coord],
    hist: &EventHistory,
    cell_idx: usize,
    universe_idx: usize,
    energy: f64,
    weight: f64,
    surface: usize,
    r_flight: Position,
    u: Direction,
    d: f64,
    leaked: bool,
    cell_instance: Option<usize>,
) {
    if !scores_crossings(tally) {
        return;
    }
    let sign = crossing_sign(geom, levels, surface, d);
    let end = advance(r_flight, u, d);
    let ev = FilterEvent {
        cell_idx,
        material_idx: hist.material.unwrap_or(usize::MAX),
        universe_idx,
        energy,
        surface_idx: surface,
        position_last: r_flight,
        position: end,
        direction: u,
        n_collision: hist.n_collision,
        cell_from: Some(cell_idx),
        cell_born: hist.cell_born,
        material_from: if leaked { hist.material_last } else { hist.material },
        surface_mu: crossing_mu(geom, surface, end, u, sign),
        position_born: hist.position_born,
        weight,
        cell_instance,
        ..Default::default()
    };
    let mut bin = 0usize;
    for f in &tally.filters {
        match f.get_bin(&ev) {
            Some(b) => bin = bin * f.n_bins() + b,
            None => return,
        }
    }
    let n_scores = tally.scores.len();
    for (s, sc) in tally.scores.iter().enumerate() {
        if matches!(sc, ScoreType::Current) {
            batch[bin * n_scores + s] += weight * sign;
        }
    }
}

/// Score the faces one flight crosses into `batch` for a tally whose only
/// filter is a mesh-surface filter and whose score is `Current`
/// (`filter_meshsurface.cpp`); any other tally is left alone. `crossing` is the
/// surface the flight ended on (`None` for a collision or a lattice boundary),
/// `collides` whether it ended in a collision.
#[allow(clippy::too_many_arguments)]
pub fn score_mesh_surface_segment(
    batch: &mut [f64],
    tally: &Tally,
    geom: &Geometry,
    hist: &EventHistory,
    cell_idx: usize,
    energy: f64,
    weight: f64,
    crossing: Option<usize>,
    collides: bool,
    r_flight: Position,
    u: Direction,
    d: f64,
) {
    let [FilterKind::MeshSurface(f)] = tally.filters.as_slice() else { return };
    let leaks = !collides
        && crossing.is_some_and(|sf| {
            matches!(geom.surfaces[sf].bc(), crate::geometry::surface::BoundaryType::Vacuum)
        });
    let (start, track_end) = flight_track_ends(r_flight, u, d, hist.after_boundary, leaks);
    let ev = FilterEvent {
        cell_idx,
        energy,
        position_last: start,
        position: track_end,
        direction: u,
        n_collision: hist.n_collision,
        cell_born: hist.cell_born,
        position_born: hist.position_born,
        weight,
        ..Default::default()
    };
    let n_scores = tally.scores.len();
    for b in f.matches(&ev) {
        for (s, sc) in tally.scores.iter().enumerate() {
            if matches!(sc, ScoreType::Current) {
                batch[b * n_scores + s] += weight;
            }
        }
    }
}

/// Rebuild the tallying events of one history from its [`TraceEvent`] stream.
///
/// `tables` are the distribcell offset tables, one per target cell; the table
/// whose `target` equals the located cell supplies `cell_instance`. Pass `&[]`
/// when no per-instance filter is used.
pub fn history_events(
    geom: &Geometry,
    trace: &[TraceEvent],
    tables: &[DistribcellOffsets],
) -> Vec<HistoryEvent> {
    let mut out: Vec<HistoryEvent> = Vec::new();
    let mut u = Direction { u: 0.0, v: 0.0, w: 1.0 };
    let mut e = 0.0f64;
    let mut w = 1.0f64;
    let mut r_born = Position::ZERO;
    let mut cell_born: Option<usize> = None;
    let mut n_coll = 0u32;
    // Current located state.
    let mut r = Position::ZERO;
    let mut cell = usize::MAX;
    let mut material: Option<usize> = None;
    let mut universe = usize::MAX;
    let mut instance: Option<usize> = None;
    // Pending segment (surface, segment start, segment end).
    let mut pending: Option<(usize, Position, Position, f64)> = None;
    let mut levels: Vec<Coord> = Vec::new();
    // OpenMC's `material_last` (`geometry.cpp:176`, `particle.cpp:392`): set to
    // the previous material at every cell search, cleared by a collision.
    let mut material_last: Option<usize> = None;
    // True when the current flight began at a boundary the transport nudged past.
    let mut after_boundary = false;

    for t in trace {
        match *t {
            TraceEvent::Start { r: r0, u: u0, e: e0 } => {
                r_born = r0;
                u = u0;
                e = e0;
            }
            TraceEvent::Located { r: r1, cell: c, material: m, .. } => {
                r = r1;
                cell = c;
                material_last = material;
                material = m;
                universe = usize::MAX;
                instance = None;
                levels.clear();
                if let Some(path) = geom.locate(r1, u, SurfaceToken::NONE) {
                    universe = path.leaf().universe;
                    levels = path.levels.clone();
                    instance = tables
                        .iter()
                        .find(|tb| tb.target == c)
                        .and_then(|tb| tb.instance_of(geom, &path.levels));
                }
                if cell_born.is_none() {
                    cell_born = Some(c);
                }
            }
            TraceEvent::Segment { d_collision, d_boundary, surface, .. } => {
                let collides = d_collision < d_boundary;
                let d = d_collision.min(d_boundary);
                let end = advance(r, u, d);
                let leaks = !collides
                    && surface.is_some_and(|sf| {
                        matches!(geom.surfaces[sf].bc(), crate::geometry::surface::BoundaryType::Vacuum)
                    });
                let track_end = if leaks { advance(end, u, TINY_BIT) } else { end };
                let start = if after_boundary { advance(r, u, -TRANSPORT_NUDGE) } else { r };
                let ev = FilterEvent {
                    cell_idx: cell,
                    material_idx: material.unwrap_or(usize::MAX),
                    universe_idx: universe,
                    energy: e,
                    position_last: start,
                    position: advance(r, u, 0.5 * d),
                    direction: u,
                    n_collision: n_coll,
                    cell_born,
                    position_born: r_born,
                    weight: w,
                    cell_instance: instance,
                    ..Default::default()
                };
                out.push(HistoryEvent { kind: HistoryEventKind::Segment { distance: d }, track_end, ev });
                // The next flight starts at a boundary unless a collision ended this one.
                after_boundary = !collides;
                pending = match (collides, surface) {
                    (false, Some(s)) => Some((s, r, end, crossing_sign(geom, &levels, s, d))),
                    _ => None,
                };
            }
            TraceEvent::Collision { r: rc, .. } => {
                let ev = FilterEvent {
                    cell_idx: cell,
                    material_idx: material.unwrap_or(usize::MAX),
                    universe_idx: universe,
                    energy: e,
                    position_last: rc,
                    position: rc,
                    direction: u,
                    n_collision: n_coll,
                    cell_born,
                    position_born: r_born,
                    weight: w,
                    cell_instance: instance,
                    ..Default::default()
                };
                out.push(HistoryEvent { kind: HistoryEventKind::Collision { scatter: false }, track_end: ev.position, ev });
                after_boundary = false;
                n_coll += 1;
                material_last = None;
                pending = None;
            }
            TraceEvent::State(s) => {
                if matches!(s.event, TrackEvent::SurfaceCrossing | TrackEvent::Leak) {
                    if let Some((surf, start, end, sign)) = pending.take() {
                        let mu = crossing_mu(geom, surf, end, u, sign);
                        // A surface crossing finds the next cell, which sets
                        // `material_last` to the material left; a vacuum leak
                        // finds none and keeps what the last search left.
                        let material_from =
                            if s.event == TrackEvent::Leak { material_last } else { material };
                        let ev = FilterEvent {
                            cell_idx: cell,
                            material_idx: material.unwrap_or(usize::MAX),
                            universe_idx: universe,
                            energy: e,
                            surface_idx: surf,
                            position_last: start,
                            position: end,
                            direction: u,
                            n_collision: n_coll,
                            cell_from: Some(cell),
                            cell_born,
                            material_from,
                            surface_mu: mu,
                            position_born: r_born,
                            weight: w,
                            cell_instance: instance,
                            ..Default::default()
                        };
                        out.push(HistoryEvent { kind: HistoryEventKind::Crossing { sign }, track_end: end, ev });
                    }
                }
                if s.event == TrackEvent::Scatter {
                    if let Some(last) = out.last_mut() {
                        if let HistoryEventKind::Collision { scatter } = &mut last.kind {
                            *scatter = true;
                            // Lab-frame scattering cosine `p.mu()`: pre- vs post-collision direction.
                            last.ev.mu = (u.u * s.u.u + u.v * s.u.v + u.w * s.u.w).clamp(-1.0, 1.0);
                            last.ev.energy_out = Some(s.energy);
                        }
                    }
                }
                u = s.u;
                e = s.energy;
                w = s.weight;
            }
            TraceEvent::Flight { .. } | TraceEvent::Tentative { .. } => {}
        }
    }
    out
}

/// Split a straight flight `r0 -> r1` among the elements of a regular mesh:
/// `(element, fraction of the flight length)`, in order of travel, stopping at
/// the mesh edge. This is the track-length splitting OpenMC's
/// `MeshFilter::get_all_bins` / `MeshMaterialFilter::get_all_bins` do through
/// `mesh.bins_crossed`; the midpoint rule the single-bin
/// [`Filter::get_bin`](crate::tally::filter) path uses is exact only for a
/// flight that stays inside one element.
pub fn regular_mesh_split(mesh: &RegularMesh, r0: Position, r1: Position) -> Vec<(usize, f64)> {
    let w = mesh.width();
    let a = [r0.x, r0.y, r0.z];
    let d = [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z];
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    for k in 0..3 {
        if d[k] == 0.0 || !(w[k] > 0.0) {
            continue;
        }
        for i in 0..=mesh.dimension[k] {
            let plane = mesh.lower_left[k] + i as f64 * w[k];
            let t = (plane - a[k]) / d[k];
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
    }
    ts.sort_by(|p, q| p.partial_cmp(q).unwrap_or(core::cmp::Ordering::Equal));
    let mut out: Vec<(usize, f64)> = Vec::new();
    for pair in ts.windows(2) {
        let frac = pair[1] - pair[0];
        if frac <= 0.0 {
            continue;
        }
        let tm = 0.5 * (pair[0] + pair[1]);
        let mid = Position { x: a[0] + tm * d[0], y: a[1] + tm * d[1], z: a[2] + tm * d[2] };
        if let Some(e) = mesh.get_bin(mid) {
            out.push((e, frac));
        }
    }
    out
}

/// Add one event's contribution to `out` (`tally.bins.len()` entries, laid out
/// `[filter_bin * n_scores + score]`). Returns an error for a tally this driver
/// does not support (see the module docs) so that nothing is silently dropped.
pub fn score_history_event(
    tally: &Tally,
    he: &HistoryEvent,
    out: &mut [f64],
) -> Result<(), String> {
    let n_scores = tally.scores.len();
    let ms = tally.filters.iter().position(|f| matches!(f, FilterKind::MeshSurface(_)));
    if let Some(i) = ms {
        if tally.filters.len() != 1 {
            return Err("a mesh-surface tally must carry no other filter here".into());
        }
        let FilterKind::MeshSurface(f) = &tally.filters[i] else { unreachable!() };
        if let HistoryEventKind::Segment { .. } = he.kind {
            let mut ev = he.ev.clone();
            ev.position = he.track_end;
            for b in f.matches(&ev) {
                for (s, sc) in tally.scores.iter().enumerate() {
                    match sc {
                        ScoreType::Current => out[b * n_scores + s] += he.ev.weight,
                        _ => return Err(format!("score {sc:?} unsupported on mesh-surface")),
                    }
                }
            }
        }
        return Ok(());
    }
    // A lone expansion filter (Legendre, ...) deposits `w * P_n(mu)` into every
    // moment bin on a scattering collision (`filter_legendre.cpp`, analog).
    if tally.filters.len() == 1 {
        if let HistoryEventKind::Collision { scatter } = he.kind {
            if let Some(moments) = tally.filters[0].expansion_moments(&he.ev) {
                if scatter {
                    for (s, sc) in tally.scores.iter().enumerate() {
                        match sc {
                            ScoreType::Scatter => {
                                for (n, pn) in moments.iter().enumerate() {
                                    out[n * n_scores + s] += he.ev.weight * pn;
                                }
                            }
                            _ => return Err(format!("score {sc:?} unsupported with an expansion filter")),
                        }
                    }
                }
                return Ok(());
            }
        }
    }
    let mm = tally.filters.iter().position(|f| matches!(f, FilterKind::MeshMaterial(_)));
    if let Some(i) = mm {
        if tally.filters.len() != 1 {
            return Err("a mesh-material tally must carry no other filter here".into());
        }
        let FilterKind::MeshMaterial(f) = &tally.filters[i] else { unreachable!() };
        let MeshKind::Regular(mesh) = &f.mesh else {
            return Err("track-length splitting is implemented for regular meshes only".into());
        };
        if let HistoryEventKind::Segment { distance } = he.kind {
            let half = 0.5 * distance;
            let u = he.ev.direction;
            let r0 = advance(he.ev.position, u, -half);
            let r1 = advance(he.ev.position, u, half);
            for (elem, frac) in regular_mesh_split(mesh, r0, r1) {
                if let Some(b) = f.pairs.iter().position(|&(e, m)| e == elem && m == he.ev.material_idx) {
                    for (s, sc) in tally.scores.iter().enumerate() {
                        match sc {
                            ScoreType::Flux => out[b * n_scores + s] += he.ev.weight * distance * frac,
                            _ => return Err(format!("score {sc:?} unsupported on mesh-material")),
                        }
                    }
                }
            }
        }
        return Ok(());
    }
    let mut bin = 0usize;
    for f in &tally.filters {
        match f.get_bin(&he.ev) {
            Some(b) => bin = bin * f.n_bins() + b,
            None => return Ok(()),
        }
    }
    let has_surface = tally.filters.iter().any(|f| matches!(f, FilterKind::Surface(_)));
    for (s, sc) in tally.scores.iter().enumerate() {
        let v = match (sc, he.kind) {
            (ScoreType::Flux, HistoryEventKind::Segment { distance }) => he.ev.weight * distance,
            (ScoreType::Current, HistoryEventKind::Crossing { sign }) if has_surface => {
                he.ev.weight * sign
            }
            (ScoreType::Scatter, HistoryEventKind::Collision { scatter: true }) => he.ev.weight,
            (ScoreType::Flux | ScoreType::Current | ScoreType::Scatter, _) => 0.0,
            _ => return Err(format!("score {sc:?} unsupported by the history driver")),
        };
        out[bin * n_scores + s] += v;
    }
    Ok(())
}

/// Score a whole history's events into `out`.
pub fn score_history(
    tally: &Tally,
    events: &[HistoryEvent],
    out: &mut [f64],
) -> Result<(), String> {
    for he in events {
        score_history_event(tally, he, out)?;
    }
    Ok(())
}
