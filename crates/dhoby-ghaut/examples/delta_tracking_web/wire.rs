//! What crosses from the physics worker to the page, flattened to `f64`s, and
//! the one-line description of a step the panes print.
//!
//! The steps travel as the library's own [`TraceEvent`]s, [`PER_EVENT`]
//! numbers each; the round trip is pinned bit for bit by the tests, so the
//! page draws exactly the steps the library took.

// The wire serves the browser build and the tests; the native GUI's
// engine thread passes values, not numbers.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::model;
use crate::physics::{GenResult, Method, RunConfig, Trace, VOID};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::physics::delta_tracking::flight::TentativeSite;
use outram_mc_libs::physics::track_output::{TrackEvent, TrackState};
use outram_mc_libs::physics::tracking_trace::{TraceCounts, TraceEvent};

/// Numbers per event on the wire.
pub const PER_EVENT: usize = 15;

/// Nuclide names, in `model::NuclearData::assemble`'s order.
pub const NUCLIDE_NAMES: [&str; 12] = [
    "U-235",
    "U-238",
    "O-16",
    "B-10",
    "B-11",
    "C-12 (graphite)",
    "C-13 (graphite)",
    "C-12",
    "C-13",
    "Si-28",
    "Si-29",
    "Si-30",
];

/// A material's name, helium included.
pub fn material_name(m: usize) -> &'static str {
    if m == VOID {
        "helium (void)"
    } else {
        model::MATERIAL_NAMES.get(m).copied().unwrap_or("?")
    }
}

fn opt(x: Option<f64>) -> f64 {
    x.unwrap_or(f64::NAN)
}
fn from_opt(x: f64) -> Option<f64> {
    (!x.is_nan()).then_some(x)
}
fn idx(x: Option<usize>) -> f64 {
    x.map_or(-1.0, |i| i as f64)
}
fn from_idx(x: f64) -> Option<usize> {
    (x >= 0.0).then_some(x as usize)
}

fn track_code(e: TrackEvent) -> f64 {
    match e {
        TrackEvent::Born => 0.0,
        TrackEvent::SurfaceCrossing => 1.0,
        TrackEvent::Scatter => 2.0,
        TrackEvent::Fission => 3.0,
        TrackEvent::Absorption => 4.0,
        TrackEvent::Rouletted => 5.0,
        TrackEvent::Leak => 6.0,
        TrackEvent::Lost => 7.0,
    }
}

fn track_from(c: f64) -> Result<TrackEvent, String> {
    Ok(match c as i64 {
        0 => TrackEvent::Born,
        1 => TrackEvent::SurfaceCrossing,
        2 => TrackEvent::Scatter,
        3 => TrackEvent::Fission,
        4 => TrackEvent::Absorption,
        5 => TrackEvent::Rouletted,
        6 => TrackEvent::Leak,
        7 => TrackEvent::Lost,
        c => return Err(format!("unknown track event {c}")),
    })
}

/// One event as [`PER_EVENT`] numbers (unused slots are 0).
pub fn encode_event(e: &TraceEvent) -> [f64; PER_EVENT] {
    let mut v = [0.0; PER_EVENT];
    let p = |v: &mut [f64; PER_EVENT], at: usize, r: Position| {
        v[at] = r.x;
        v[at + 1] = r.y;
        v[at + 2] = r.z;
    };
    let d = |v: &mut [f64; PER_EVENT], at: usize, u: Direction| {
        v[at] = u.u;
        v[at + 1] = u.v;
        v[at + 2] = u.w;
    };
    match *e {
        TraceEvent::Start { r, u, e } => {
            v[0] = 0.0;
            p(&mut v, 1, r);
            d(&mut v, 4, u);
            v[7] = e;
        }
        TraceEvent::Located {
            r,
            cell,
            material,
            sigma_t,
        } => {
            v[0] = 1.0;
            p(&mut v, 1, r);
            v[4] = cell as f64;
            v[5] = idx(material);
            v[6] = sigma_t;
        }
        TraceEvent::Segment {
            xi,
            d_collision,
            d_boundary,
            surface,
        } => {
            v[0] = 2.0;
            v[1] = opt(xi);
            v[2] = d_collision;
            v[3] = d_boundary;
            v[4] = idx(surface);
        }
        TraceEvent::Flight {
            from,
            u,
            to,
            xi,
            distance,
            majorant,
            exited,
        } => {
            v[0] = 3.0;
            p(&mut v, 1, from);
            d(&mut v, 4, u);
            p(&mut v, 7, to);
            v[10] = xi;
            v[11] = distance;
            v[12] = majorant;
            v[13] = exited as u8 as f64;
        }
        TraceEvent::Tentative { site, xi, real } => {
            v[0] = 4.0;
            p(&mut v, 1, site.position);
            v[4] = idx(site.material);
            v[5] = site.majorant;
            v[6] = site.sigma_t;
            v[7] = opt(xi);
            v[8] = real as u8 as f64;
        }
        TraceEvent::Collision {
            r,
            material,
            nuclide,
            e,
        } => {
            v[0] = 5.0;
            p(&mut v, 1, r);
            v[4] = material as f64;
            v[5] = nuclide as f64;
            v[6] = e;
        }
        TraceEvent::State(s) => {
            v[0] = 6.0;
            p(&mut v, 1, s.r);
            d(&mut v, 4, s.u);
            v[7] = s.energy;
            v[8] = s.time;
            v[9] = s.weight;
            v[10] = if s.cell == usize::MAX {
                -1.0
            } else {
                s.cell as f64
            };
            v[11] = idx(s.material);
            v[12] = track_code(s.event);
        }
    }
    v
}

/// The inverse of [`encode_event`].
pub fn decode_event(v: &[f64]) -> Result<TraceEvent, String> {
    if v.len() != PER_EVENT {
        return Err(format!("event: {} numbers", v.len()));
    }
    let p = |at: usize| Position::new(v[at], v[at + 1], v[at + 2]);
    let d = |at: usize| Direction {
        u: v[at],
        v: v[at + 1],
        w: v[at + 2],
    };
    Ok(match v[0] as i64 {
        0 => TraceEvent::Start {
            r: p(1),
            u: d(4),
            e: v[7],
        },
        1 => TraceEvent::Located {
            r: p(1),
            cell: v[4] as usize,
            material: from_idx(v[5]),
            sigma_t: v[6],
        },
        2 => TraceEvent::Segment {
            xi: from_opt(v[1]),
            d_collision: v[2],
            d_boundary: v[3],
            surface: from_idx(v[4]),
        },
        3 => TraceEvent::Flight {
            from: p(1),
            u: d(4),
            to: p(7),
            xi: v[10],
            distance: v[11],
            majorant: v[12],
            exited: v[13] != 0.0,
        },
        4 => TraceEvent::Tentative {
            site: TentativeSite {
                position: p(1),
                material: from_idx(v[4]),
                majorant: v[5],
                sigma_t: v[6],
            },
            xi: from_opt(v[7]),
            real: v[8] != 0.0,
        },
        5 => TraceEvent::Collision {
            r: p(1),
            material: v[4] as usize,
            nuclide: v[5] as usize,
            e: v[6],
        },
        6 => TraceEvent::State(TrackState {
            r: p(1),
            u: d(4),
            energy: v[7],
            time: v[8],
            weight: v[9],
            cell: if v[10] < 0.0 {
                usize::MAX
            } else {
                v[10] as usize
            },
            material: from_idx(v[11]),
            event: track_from(v[12])?,
        }),
        k => return Err(format!("unknown event kind {k}")),
    })
}

const COUNTS: usize = 12;

pub fn encode_counts(c: &TraceCounts) -> [f64; COUNTS] {
    [
        c.starts as f64,
        c.locates as f64,
        c.boundary_queries as f64,
        c.crossings as f64,
        c.flights as f64,
        c.tentative as f64,
        c.virtual_collisions as f64,
        c.majorant_violations as f64,
        c.collisions as f64,
        c.fissions as f64,
        c.captures as f64,
        c.leaked_or_lost as f64,
    ]
}

pub fn decode_counts(v: &[f64]) -> Result<TraceCounts, String> {
    if v.len() != COUNTS {
        return Err(format!("counts: {} numbers", v.len()));
    }
    let u = |i: usize| v[i] as u64;
    Ok(TraceCounts {
        starts: u(0),
        locates: u(1),
        boundary_queries: u(2),
        crossings: u(3),
        flights: u(4),
        tentative: u(5),
        virtual_collisions: u(6),
        majorant_violations: u(7),
        collisions: u(8),
        fissions: u(9),
        captures: u(10),
        leaked_or_lost: u(11),
    })
}

/// A pane's trace: counts, the number dropped, then the events.
pub fn encode_trace(t: &Trace) -> Vec<f64> {
    let mut v = Vec::with_capacity(COUNTS + 2 + PER_EVENT * t.events.len());
    v.extend(encode_counts(&t.counts));
    v.push(t.dropped as f64);
    v.push(t.events.len() as f64);
    for e in &t.events {
        v.extend(encode_event(e));
    }
    v
}

pub fn decode_trace(v: &[f64]) -> Result<Trace, String> {
    if v.len() < COUNTS + 2 {
        return Err("trace too short".into());
    }
    let n = v[COUNTS + 1] as usize;
    let body = &v[COUNTS + 2..];
    if body.len() != n * PER_EVENT {
        return Err(format!("trace: {} numbers for {n} events", body.len()));
    }
    Ok(Trace {
        counts: decode_counts(&v[..COUNTS])?,
        dropped: v[COUNTS] as usize,
        events: body
            .chunks_exact(PER_EVENT)
            .map(decode_event)
            .collect::<Result<_, _>>()?,
    })
}

pub fn encode_gen(g: &GenResult) -> Vec<f64> {
    let (m, s) = g.k_mean.unwrap_or((f64::NAN, f64::NAN));
    let mut v = vec![
        g.method.code(),
        g.index as f64,
        g.active as u8 as f64,
        g.k,
        m,
        s,
        g.secs,
        g.n_particles as f64,
        g.last as u8 as f64,
    ];
    v.extend(encode_counts(&g.counts));
    v
}

pub fn decode_gen(v: &[f64]) -> Result<GenResult, String> {
    if v.len() != 9 + COUNTS {
        return Err(format!("generation: {} numbers", v.len()));
    }
    Ok(GenResult {
        method: Method::from_code(v[0])?,
        index: v[1] as usize,
        active: v[2] != 0.0,
        k: v[3],
        k_mean: from_opt(v[4]).map(|m| (m, v[5])),
        secs: v[6],
        n_particles: v[7] as usize,
        last: v[8] != 0.0,
        counts: decode_counts(&v[9..])?,
    })
}

pub fn encode_run(c: &RunConfig) -> [f64; 5] {
    [
        c.n_particles as f64,
        c.n_inactive as f64,
        c.n_active as f64,
        c.seed as f64,
        opt(c.low_factor),
    ]
}

pub fn decode_run(v: &[f64]) -> Result<RunConfig, String> {
    if v.len() != 5 {
        return Err(format!("run config: {} numbers", v.len()));
    }
    Ok(RunConfig {
        n_particles: v[0] as usize,
        n_inactive: v[1] as usize,
        n_active: v[2] as usize,
        seed: v[3] as u64,
        low_factor: from_opt(v[4]),
    })
}

/// Energy as the panes print it.
pub fn energy(e: f64) -> String {
    if e >= 1.0e6 {
        format!("{:.3} MeV", e / 1.0e6)
    } else if e >= 1.0e3 {
        format!("{:.3} keV", e / 1.0e3)
    } else if e >= 1.0 {
        format!("{:.3} eV", e)
    } else {
        format!("{:.4} eV", e)
    }
}

/// A length as the panes print it.
pub fn length(d: f64) -> String {
    if !d.is_finite() {
        "∞".into()
    } else if d < 0.1 {
        format!("{:.1} µm", d * 1.0e4)
    } else {
        format!("{d:.3} cm")
    }
}

/// One line saying what a step did.
pub fn describe(e: &TraceEvent) -> String {
    match *e {
        TraceEvent::Start { e, .. } => format!("Born at {}", energy(e)),
        TraceEvent::Located {
            cell,
            material,
            sigma_t,
            ..
        } => format!(
            "locate -> cell {cell}: {} (Σt = {sigma_t:.3} /cm)",
            material.map_or("helium (void)", material_name)
        ),
        TraceEvent::Segment {
            xi,
            d_collision,
            d_boundary,
            surface,
        } => {
            let what = if d_collision < d_boundary {
                "collide first"
            } else {
                "surface first: move to it"
            };
            let s = surface.map_or("none".into(), |s| format!("#{s}"));
            match xi {
                Some(x) => format!(
                    "ξ = {x:.3} -> d_col = −ln ξ/Σt = {}; nearest surface {s} at {}: {what}",
                    length(d_collision),
                    length(d_boundary)
                ),
                None => format!("void: no ξ; nearest surface {s} at {}", length(d_boundary)),
            }
        }
        TraceEvent::Flight {
            xi,
            distance,
            majorant,
            exited,
            ..
        } => format!(
            "ξ = {xi:.3} -> s = −ln ξ/Σmaj = {} (Σmaj = {majorant:.3} /cm){}",
            length(distance),
            if exited {
                ", leaves the region"
            } else {
                "; no surface looked for"
            }
        ),
        TraceEvent::Tentative { site, xi, real } => {
            let m = site.material.map_or("?", material_name);
            let ratio = site.sigma_t / site.majorant;
            let over = if site.violates_majorant() {
                "  ⚠ Σt > Σmaj: bound broken"
            } else {
                ""
            };
            match xi {
                Some(x) => format!(
                    "{m}: Σt/Σmaj = {ratio:.3}; ξ = {x:.3} {} -> {}{over}",
                    if real { "<" } else { "≥" },
                    if real {
                        "REAL collision"
                    } else {
                        "virtual, fly on"
                    }
                ),
                None => format!("{m}: Σt = 0 -> virtual, fly on"),
            }
        }
        TraceEvent::Collision {
            material,
            nuclide,
            e,
            ..
        } => format!(
            "collision in {} with {} at {}",
            material_name(material),
            NUCLIDE_NAMES.get(nuclide).copied().unwrap_or("?"),
            energy(e)
        ),
        TraceEvent::State(s) => match s.event {
            TrackEvent::SurfaceCrossing => "crossed the surface; locate again".into(),
            TrackEvent::Scatter => format!("scattered -> {}", energy(s.energy)),
            TrackEvent::Fission => "fission: the history ends".into(),
            TrackEvent::Absorption => "captured: the history ends".into(),
            TrackEvent::Leak => "leaked".into(),
            TrackEvent::Lost => "lost".into(),
            TrackEvent::Born | TrackEvent::Rouletted => format!("{:?}", s.event),
        },
    }
}
