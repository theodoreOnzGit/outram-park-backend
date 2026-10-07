//! The page's state, apart from drawing: where each pane's cursor stands in
//! its neutron's steps, what has been drawn so far, and the "Run many"
//! bookkeeping (which generation to ask for next, the running results).
//! Kept out of `app.rs` so it is tested headlessly.

use crate::physics::{GenResult, Method, RunConfig, Trace};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::pebble_beds::keff_delta::DeltaDomain;
use outram_mc_libs::physics::track_output::TrackEvent;
use outram_mc_libs::physics::tracking_trace::{TraceCounts, TraceEvent};

/// How a piece of track is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegKind {
    /// Surface tracking: a segment that ended on a surface.
    ToSurface,
    /// Surface tracking: a segment that ended in a collision.
    ToCollision,
    /// Delta tracking: one flight on the majorant.
    Flight,
}

/// A piece of track, drawn once the cursor has passed event `at`.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub points: Vec<[f64; 2]>,
    pub kind: SegKind,
    pub at: usize,
}

/// A point marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Birth,
    /// A surface crossing (surface tracking).
    Crossing,
    /// A virtual collision (delta tracking).
    Virtual,
    /// A tentative site where the majorant failed to bound `Σ_t`.
    Violation,
    /// A real collision (both).
    Real,
    /// The history ended here.
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub p: [f64; 2],
    pub kind: MarkKind,
    pub at: usize,
}

fn xy(r: Position) -> [f64; 2] {
    [r.x, r.y]
}

/// One pane's neutron, and how far through it the reader has stepped.
#[derive(Clone, Debug)]
pub struct PaneTrace {
    pub trace: Trace,
    /// Events shown: `events[..cursor]`.
    pub cursor: usize,
    pub segs: Vec<Seg>,
    pub marks: Vec<Mark>,
    /// Position (x, y) and direction after each event.
    pos: Vec<[f64; 2]>,
    dir: Vec<Direction>,
}

impl PaneTrace {
    /// Lay out a trace for drawing. `domain` is the delta-tracking domain, to
    /// draw a flight that reflects off a wall as the library flew it
    /// ([`DeltaDomain::advance`]).
    pub fn new(trace: Trace, domain: DeltaDomain) -> Self {
        let n = trace.events.len();
        let (mut pos, mut dir) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let (mut segs, mut marks) = (Vec::new(), Vec::new());
        let mut here = [0.0, 0.0];
        let mut u = Direction::new(1.0, 0.0, 0.0);
        let mut seg_end: Option<SegKind> = None;
        for (i, e) in trace.events.iter().enumerate() {
            match *e {
                TraceEvent::Start { r, u: d, .. } => {
                    here = xy(r);
                    u = d;
                    marks.push(Mark {
                        p: here,
                        kind: MarkKind::Birth,
                        at: i,
                    });
                }
                TraceEvent::Located { r, .. } => here = xy(r),
                TraceEvent::Segment {
                    d_collision,
                    d_boundary,
                    ..
                } => {
                    seg_end = Some(if d_collision < d_boundary {
                        SegKind::ToCollision
                    } else {
                        SegKind::ToSurface
                    });
                }
                TraceEvent::Flight {
                    from,
                    u: d,
                    to,
                    distance,
                    ..
                } => {
                    let straight = Position::new(
                        from.x + d.u * distance,
                        from.y + d.v * distance,
                        from.z + d.w * distance,
                    );
                    let points = if ((straight.x - to.x).powi(2)
                        + (straight.y - to.y).powi(2)
                        + (straight.z - to.z).powi(2))
                    .sqrt()
                        < 1e-9
                    {
                        vec![xy(from), xy(to)]
                    } else {
                        // It reflected on the way: draw it as the domain flew it.
                        (0..=48)
                            .map(|k| xy(domain.advance(from, d, distance * k as f64 / 48.0).0))
                            .collect()
                    };
                    segs.push(Seg {
                        points,
                        kind: SegKind::Flight,
                        at: i,
                    });
                    here = xy(to);
                }
                TraceEvent::Tentative { site, real, .. } => {
                    let kind = if site.violates_majorant() {
                        MarkKind::Violation
                    } else if real {
                        MarkKind::Real
                    } else {
                        MarkKind::Virtual
                    };
                    marks.push(Mark {
                        p: xy(site.position),
                        kind,
                        at: i,
                    });
                }
                TraceEvent::Collision { r, .. } => {
                    if let Some(kind) = seg_end.take() {
                        segs.push(Seg {
                            points: vec![here, xy(r)],
                            kind,
                            at: i,
                        });
                        marks.push(Mark {
                            p: xy(r),
                            kind: MarkKind::Real,
                            at: i,
                        });
                    }
                    here = xy(r);
                }
                TraceEvent::State(s) => {
                    if s.event == TrackEvent::SurfaceCrossing {
                        if let Some(kind) = seg_end.take() {
                            segs.push(Seg {
                                points: vec![here, xy(s.r)],
                                kind,
                                at: i,
                            });
                        }
                        marks.push(Mark {
                            p: xy(s.r),
                            kind: MarkKind::Crossing,
                            at: i,
                        });
                    }
                    if matches!(
                        s.event,
                        TrackEvent::Fission
                            | TrackEvent::Absorption
                            | TrackEvent::Leak
                            | TrackEvent::Lost
                    ) {
                        marks.push(Mark {
                            p: xy(s.r),
                            kind: MarkKind::End,
                            at: i,
                        });
                    }
                    here = xy(s.r);
                    u = s.u;
                }
            }
            pos.push(here);
            dir.push(u);
        }
        Self {
            trace,
            cursor: 0,
            segs,
            marks,
            pos,
            dir,
        }
    }

    pub fn len(&self) -> usize {
        self.trace.events.len()
    }
    pub fn finished(&self) -> bool {
        self.cursor >= self.len()
    }
    /// Show one more event; `false` at the end.
    pub fn step(&mut self) -> bool {
        if self.finished() {
            return false;
        }
        self.cursor += 1;
        true
    }
    /// Step until a real collision and its outcome are shown (or the end).
    pub fn next_collision(&mut self) {
        while self.step() {
            if matches!(self.current(), Some(TraceEvent::State(s)) if s.event != TrackEvent::SurfaceCrossing)
            {
                return;
            }
        }
    }
    pub fn to_end(&mut self) {
        self.cursor = self.len();
    }
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }
    /// The last event shown.
    pub fn current(&self) -> Option<&TraceEvent> {
        self.cursor
            .checked_sub(1)
            .and_then(|i| self.trace.events.get(i))
    }
    /// Where the neutron is after the last event shown (the birth point
    /// before any).
    pub fn position(&self) -> [f64; 2] {
        match self.cursor.checked_sub(1) {
            Some(i) => self.pos[i],
            None => self.pos.first().copied().unwrap_or([0.0, 0.0]),
        }
    }
    pub fn direction(&self) -> Direction {
        match self.cursor.checked_sub(1) {
            Some(i) => self.dir[i],
            None => self
                .dir
                .first()
                .copied()
                .unwrap_or(Direction::new(1.0, 0.0, 0.0)),
        }
    }
    /// The counts of the events shown so far.
    pub fn counts(&self) -> TraceCounts {
        let mut c = TraceCounts::default();
        for e in &self.trace.events[..self.cursor] {
            c.add(e);
        }
        c
    }
}

/// The step-by-step view: the same neutron in both panes.
#[derive(Clone, Debug)]
pub struct Pair {
    pub seed: u64,
    pub factor: f64,
    pub surface: PaneTrace,
    pub delta: PaneTrace,
    /// Events per second while playing, and the fraction of an event owed.
    pub playing: bool,
    owed: f64,
}

impl Pair {
    pub fn new(seed: u64, factor: f64, surface: Trace, delta: Trace, domain: DeltaDomain) -> Self {
        Self {
            seed,
            factor,
            surface: PaneTrace::new(surface, domain),
            delta: PaneTrace::new(delta, domain),
            playing: false,
            owed: 0.0,
        }
    }
    /// One event in each pane.
    pub fn step(&mut self) {
        self.surface.step();
        self.delta.step();
    }
    pub fn next_collision(&mut self) {
        self.surface.next_collision();
        self.delta.next_collision();
    }
    pub fn to_end(&mut self) {
        self.surface.to_end();
        self.delta.to_end();
        self.playing = false;
    }
    pub fn rewind(&mut self) {
        self.surface.rewind();
        self.delta.rewind();
        self.playing = false;
    }
    pub fn finished(&self) -> bool {
        self.surface.finished() && self.delta.finished()
    }
    /// Advance `dt` seconds of play at `rate` events per second; returns
    /// whether anything moved. Stops at the end.
    pub fn play(&mut self, dt: f64, rate: f64) -> bool {
        if !self.playing {
            return false;
        }
        self.owed += dt * rate;
        let n = self.owed.floor() as usize;
        self.owed -= n as f64;
        for _ in 0..n {
            self.step();
        }
        if self.finished() {
            self.playing = false;
        }
        n > 0
    }
}

// ─── Run many ────────────────────────────────────────────────────────────────

/// One method's results so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tally {
    pub generations: usize,
    pub histories: usize,
    pub secs: f64,
    pub counts: TraceCounts,
    /// Active-generation mean and standard error.
    pub k: Option<(f64, f64)>,
    pub done: bool,
}

impl Tally {
    pub fn us_per_history(&self) -> f64 {
        if self.histories == 0 {
            f64::NAN
        } else {
            1.0e6 * self.secs / self.histories as f64
        }
    }
    /// A count per history.
    pub fn per_history(&self, n: u64) -> f64 {
        n as f64 / self.histories.max(1) as f64
    }
}

/// A "Run many": which generation to ask for next, and the results.
#[derive(Clone, Debug)]
pub struct ManyRun {
    pub cfg: RunConfig,
    pub methods: Vec<Method>,
    pub tallies: Vec<Tally>,
    /// Each method's k trace (generation index, k, running mean and σ).
    pub k_trace: Vec<Vec<(usize, f64, Option<(f64, f64)>)>>,
    in_flight: bool,
    next: usize,
    pub running: bool,
}

impl ManyRun {
    pub fn new(cfg: RunConfig) -> Self {
        let methods: Vec<Method> = if cfg.low_factor.is_some() {
            Method::ALL.to_vec()
        } else {
            Method::ALL[..2].to_vec()
        };
        let n = methods.len();
        Self {
            cfg,
            methods,
            tallies: vec![Tally::default(); n],
            k_trace: vec![Vec::new(); n],
            in_flight: false,
            next: 0,
            running: true,
        }
    }
    pub fn total_generations(&self) -> usize {
        self.cfg.n_inactive + self.cfg.n_active
    }
    /// The next generation to ask the engine for (round robin over the
    /// methods not yet done), or `None` while one is running or all are done.
    pub fn next_request(&mut self) -> Option<Method> {
        if self.in_flight || !self.running {
            return None;
        }
        for k in 0..self.methods.len() {
            let i = (self.next + k) % self.methods.len();
            if !self.tallies[i].done {
                self.next = (i + 1) % self.methods.len();
                self.in_flight = true;
                return Some(self.methods[i]);
            }
        }
        self.running = false;
        None
    }
    pub fn receive(&mut self, g: GenResult) {
        self.in_flight = false;
        let Some(i) = self.methods.iter().position(|&m| m == g.method) else {
            return;
        };
        let t = &mut self.tallies[i];
        t.generations += 1;
        t.histories += g.n_particles;
        t.secs += g.secs;
        t.counts.merge(&g.counts);
        if g.k_mean.is_some() {
            t.k = g.k_mean;
        }
        t.done = g.last || t.generations >= self.cfg.n_inactive + self.cfg.n_active;
        self.k_trace[i].push((g.index, g.k, g.k_mean));
        if self.tallies.iter().all(|t| t.done) {
            self.running = false;
        }
    }
    /// A failed request: stop rather than ask again.
    pub fn failed(&mut self) {
        self.in_flight = false;
        self.running = false;
    }
    pub fn tally(&self, m: Method) -> Option<&Tally> {
        self.methods
            .iter()
            .position(|&x| x == m)
            .map(|i| &self.tallies[i])
    }
    pub fn all_done(&self) -> bool {
        self.tallies.iter().all(|t| t.done)
    }
    /// `(Δk in pcm, σ in pcm, z)` of method `a` against `b`, once both have
    /// an active mean.
    pub fn difference(&self, a: Method, b: Method) -> Option<(f64, f64, f64)> {
        let (ka, sa) = self.tally(a)?.k?;
        let (kb, sb) = self.tally(b)?.k?;
        let s = (sa * sa + sb * sb).sqrt();
        // One active generation has no spread yet: no σ to compare with.
        (s > 0.0).then(|| ((ka - kb) * 1e5, s * 1e5, (ka - kb) / s))
    }
}

/// A prediction the reader commits to before the answer is shown.
#[derive(Clone, Debug)]
pub struct Predict {
    pub question: &'static str,
    pub options: &'static [&'static str],
    pub chosen: Option<usize>,
}

impl Predict {
    pub const fn new(question: &'static str, options: &'static [&'static str]) -> Self {
        Self {
            question,
            options,
            chosen: None,
        }
    }
    /// "You predicted …", once an option is chosen.
    pub fn verdict(&self) -> Option<String> {
        self.chosen
            .and_then(|i| self.options.get(i))
            .map(|o| format!("You predicted: {o}."))
    }
}
