//! The TRISO-ATOPS demo's app: a side panel of controls (`web_demo::panel`),
//! a main view that is either the particle (`web_demo::view`, world unit µm)
//! or a stack of plots, and a link to the engine (`web_demo::link`) that does
//! every calculation off this thread. Each frame only drains the link, draws
//! and sends.

use crate::engine::{
    Event, Frame, Request, Series, DECAY_NUCLIDES, GEOMETRIES, LAYER_NUCLIDES, POPULATION,
    rel, release_dims, release_header, release_value, GROUPS, RELEASE_SKIPPED,
};
use crate::rungs::Rung;
use dhoby_ghaut::web_demo::lesson::{self, Rung as _};
use dhoby_ghaut::web_demo::link::Link;
use dhoby_ghaut::web_demo::panel::Panel;
use dhoby_ghaut::web_demo::platform::{now_s, set_query, set_title};
use dhoby_ghaut::web_demo::view::{apply_zoom, zoom_buttons, View, Zoom};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

type TLink = Link<Request, Event>;

/// Series colours: blue, orange, green, purple, grey, brown.
const PALETTE: [Color32; 6] = [
    Color32::from_rgb(110, 170, 255),
    Color32::from_rgb(255, 170, 80),
    Color32::from_rgb(120, 220, 140),
    Color32::from_rgb(200, 140, 255),
    Color32::from_rgb(200, 200, 200),
    Color32::from_rgb(220, 150, 120),
];

/// Region colours and names, in `TrisoRegion` order (the colours of
/// boon-lay's `triso_cell_slice` images, OPyC lightened for a dark page).
const REGIONS: [(Color32, &str); 6] = [
    (Color32::from_rgb(192, 57, 43), "kernel (UO2)"),
    (Color32::from_rgb(217, 195, 140), "buffer (porous C)"),
    (Color32::from_rgb(110, 125, 140), "IPyC"),
    (Color32::from_rgb(46, 134, 193), "SiC"),
    (Color32::from_rgb(70, 90, 120), "OPyC"),
    (Color32::TRANSPARENT, "outside (matrix)"),
];

/// Species colours for rung 2, in order of first appearance.
const SPECIES: [Color32; 6] = [
    Color32::from_rgb(255, 210, 90),
    Color32::from_rgb(110, 200, 255),
    Color32::from_rgb(140, 230, 140),
    Color32::from_rgb(230, 140, 230),
    Color32::from_rgb(240, 240, 240),
    Color32::from_rgb(255, 140, 110),
];

/// Every control, for every rung (each rung reads the ones it needs).
#[derive(Clone, Copy, PartialEq)]
struct Controls {
    geometry: u8,
    decay_nuclide: u8,
    t_over_half: f64,
    playing: bool,
    walk_temp_c: f64,
    walk_dt_s: f64,
    walk_running: bool,
    layer_nuclide: u8,
    layer_temp_c: f64,
    irr_c: f64,
    hold_c: f64,
    hours: f64,
    cursor_h: f64,
    o2_kpa: f64,
    steam_kpa: f64,
    h2_kpa: f64,
    /// What the release rung shows: [`PICK_ALL`], a group
    /// (`PICK_GROUP + g`) or one nuclide (`PICK_ONE + i`).
    release_pick: u8,
    f_hm: f64,
    k_plate: f64,
    k_clean: f64,
    leak_on: bool,
    k_leak: f64,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            geometry: 0,
            decay_nuclide: 0,
            t_over_half: 0.0,
            playing: true,
            // CRP-6 Case 1a.
            walk_temp_c: 1200.0,
            walk_dt_s: 3600.0,
            walk_running: true,
            layer_nuclide: 0,
            layer_temp_c: 1600.0,
            // The HTR-Module average, the nearest published figure (a stand-in,
            // htr10::particle's doc).
            irr_c: 776.0,
            hold_c: 1800.0,
            hours: 500.0,
            cursor_h: 500.0,
            // Air.
            o2_kpa: 21.3,
            steam_kpa: 5.0,
            h2_kpa: 0.0,
            release_pick: PICK_ALL,
            // NP-MHTGR reference case (TRISO-ATOPS's shipped reference).
            f_hm: 1.0e-4,
            // Stoyer et al. 2026 Case A, Table 3 (sembawang's
            // NormalOperation::np_mhtgr_reference).
            k_plate: 7.5e-5,
            k_clean: 8.77e-5,
            leak_on: false,
            k_leak: 1.0e-5,
        }
    }
}

/// What a request depends on, so a change triggers exactly one recompute.
#[derive(Clone, Copy, PartialEq)]
enum Key {
    Slice(u8),
    Decay(u8, u64),
    Layers(u8, u64),
    Failure([u64; 4]),
    Chemistry([u64; 3]),
    Release([u64; 7]),
    None,
}

pub struct TrisoApp {
    link: Option<TLink>,
    error: Option<String>,
    rung: Rung,
    panel: Panel,
    view: View,
    c: Controls,
    shown: Key,
    next_id: u32,
    in_flight: Option<(u32, f64)>,
    frame: Option<Frame>,
    /// The CRP-6 Case 1 check, when asked for (rung 3).
    check: Option<Frame>,
    check_wanted: bool,
    walk_reset: bool,
    last_walk_at: f64,
    last_play_at: f64,
    plot_text: f32,
    title: String,
    tapped: Option<String>,
}

fn bits(x: f64) -> u64 {
    x.to_bits()
}

impl TrisoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rung: Rung) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        #[cfg(not(target_arch = "wasm32"))]
        let link: Result<TLink, String> = {
            let ctx = cc.egui_ctx.clone();
            Ok(dhoby_ghaut::web_demo::link::start_native(
                crate::engine::Engine::default(),
                move || ctx.request_repaint(),
            ))
        };
        #[cfg(target_arch = "wasm32")]
        let link = dhoby_ghaut::web_demo::link::start_web::<Request, Event>(
            cc.egui_ctx.clone(),
            "./worker.js",
            Event::Error,
        );
        let (link, error) = match link {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            link,
            error,
            rung,
            panel: Panel::default(),
            view: View::new(500.0),
            c: Controls::default(),
            shown: Key::None,
            next_id: 1,
            in_flight: None,
            frame: None,
            check: None,
            check_wanted: false,
            walk_reset: true,
            last_walk_at: 0.0,
            last_play_at: now_s(),
            plot_text: 13.0,
            title: String::new(),
            tapped: None,
        }
    }

    fn set_rung(&mut self, r: Rung) {
        if r != self.rung {
            self.rung = r;
            self.view = View::new(500.0);
            self.frame = None;
            self.shown = Key::None;
            self.walk_reset = true;
            self.tapped = None;
        }
        set_query(&[("rung", self.rung.name())]);
    }

    fn key(&self) -> Key {
        let c = &self.c;
        match self.rung {
            Rung::Triso => Key::Slice(c.geometry),
            Rung::Decay => Key::Decay(c.decay_nuclide, bits(c.t_over_half)),
            Rung::Layers => Key::Layers(c.layer_nuclide, bits(c.layer_temp_c)),
            Rung::Failure => Key::Failure([
                bits(c.irr_c),
                bits(c.hold_c),
                bits(c.hours),
                bits(c.cursor_h),
            ]),
            Rung::Chemistry => Key::Chemistry([bits(c.o2_kpa), bits(c.steam_kpa), bits(c.h2_kpa)]),
            Rung::Release => Key::Release(
                [
                    bits(c.irr_c),
                    bits(c.hold_c),
                    bits(c.hours),
                    bits(c.f_hm),
                    bits(c.k_plate),
                    bits(c.k_clean),
                    bits(if c.leak_on { c.k_leak } else { 0.0 }),
                ],
            ),
            Rung::Walk | Rung::SourceTerm => Key::None,
        }
    }

    fn request(&self, id: u32) -> Option<Request> {
        let c = self.c;
        Some(match self.rung {
            Rung::Triso => Request::Slice {
                id,
                geometry: c.geometry,
                cells: 200,
            },
            Rung::Decay => Request::Decay {
                id,
                nuclide: c.decay_nuclide,
                t_over_half: c.t_over_half,
            },
            Rung::Walk => Request::Walk {
                id,
                reset: self.walk_reset,
                temp_c: c.walk_temp_c,
                dt_s: if c.walk_running && !self.walk_reset {
                    c.walk_dt_s
                } else {
                    0.0
                },
            },
            Rung::Layers => Request::Layers {
                id,
                nuclide: c.layer_nuclide,
                temp_c: c.layer_temp_c,
            },
            Rung::Failure => Request::Failure {
                id,
                irr_c: c.irr_c,
                hold_c: c.hold_c,
                hours: c.hours,
                cursor_h: c.cursor_h,
            },
            Rung::Chemistry => Request::Chemistry {
                id,
                o2_kpa: c.o2_kpa,
                steam_kpa: c.steam_kpa,
                h2_kpa: c.h2_kpa,
            },
            Rung::Release => Request::Release {
                id,
                irr_c: c.irr_c,
                hold_c: c.hold_c,
                hours: c.hours,
                f_hm: c.f_hm,
                k_plate: c.k_plate,
                k_clean: c.k_clean,
                k_leak: if c.leak_on { c.k_leak } else { 0.0 },
            },
            Rung::SourceTerm => return None,
        })
    }

    /// Send what the picture needs, one request in flight at a time, so the
    /// worker never builds a queue the reader would wait behind.
    fn pump(&mut self) {
        let Some(link) = &self.link else { return };
        if self.in_flight.is_some() {
            return;
        }
        let now = now_s();
        let id = self.next_id;
        if self.rung == Rung::Walk && self.check_wanted {
            self.check_wanted = false;
            link.send(Request::WalkCheck {
                id,
                temp_c: self.c.walk_temp_c,
            });
            self.next_id += 1;
            self.in_flight = Some((id, now));
            return;
        }
        let wanted = match self.rung {
            // About 20 frames a second while running.
            Rung::Walk => {
                self.walk_reset
                    || self.frame.is_none()
                    || (self.c.walk_running && now - self.last_walk_at >= 0.05)
            }
            Rung::SourceTerm => false,
            _ => self.shown != self.key(),
        };
        if !wanted {
            return;
        }
        let Some(req) = self.request(id) else { return };
        link.send(req);
        self.next_id += 1;
        self.in_flight = Some((id, now));
        self.shown = self.key();
        if self.rung == Rung::Walk {
            self.last_walk_at = now;
            self.walk_reset = false;
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            match e {
                Event::Frame(f) => {
                    if self.in_flight.map(|x| x.0) == Some(f.id) {
                        self.in_flight = None;
                    }
                    // On the walk rung, the Case 1 check is the answer
                    // without the particle's radii.
                    if self.rung == Rung::Walk && f.dots.is_empty() && f.radii.is_empty() {
                        self.check = Some(f);
                    } else {
                        self.frame = Some(f);
                    }
                }
                Event::Error(m) => {
                    self.in_flight = None;
                    self.error = Some(m);
                }
            }
        }
    }

    /// Advance the animated cursors (decay time, accident cursor) on the UI
    /// side; each step is one cheap request.
    fn play(&mut self) {
        let now = now_s();
        let dt = (now - self.last_play_at).min(0.2);
        self.last_play_at = now;
        if !self.c.playing {
            return;
        }
        match self.rung {
            Rung::Decay => {
                self.c.t_over_half += dt * 0.5;
                if self.c.t_over_half > 5.0 {
                    self.c.t_over_half = 0.0;
                }
            }
            Rung::Failure | Rung::Release => {
                self.c.cursor_h += dt * self.c.hours / 10.0;
                if self.c.cursor_h > self.c.hours {
                    self.c.cursor_h = 0.0;
                }
            }
            _ => {}
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Research, education and V&V only. Not for reactor operation, licensing, safety decisions or emergency response.").small());
        ui.separator();
        let picked = lesson::picker(ui, self.rung);
        if picked != self.rung {
            self.set_rung(picked);
        }
        ui.strong(self.rung.title());
        lesson::whats_happening(ui, self.rung);
        ui.separator();
        let c = &mut self.c;
        let f = self.frame.as_ref();
        let s = |i: usize| f.and_then(|f| f.scalars.get(i).copied()).unwrap_or(f64::NAN);
        match self.rung {
            Rung::Triso => {
                ui.horizontal_wrapped(|ui| {
                    for (i, (name, _)) in GEOMETRIES.iter().enumerate() {
                        ui.selectable_value(&mut c.geometry, i as u8, *name);
                    }
                });
                ui.label("Every pixel is coloured by asking the assembled TrisoCell which region its centre is in (get_triso_region), never from the radii. Tap the particle to name a layer.");
                if let Some(f) = f {
                    egui::Grid::new("radii").striped(true).show(ui, |ui| {
                        ui.strong("interface");
                        ui.strong("radius, µm");
                        ui.end_row();
                        for (k, r) in f.radii.iter().enumerate() {
                            ui.label(format!("{} / {}", REGIONS[k].1, REGIONS[k + 1].1));
                            ui.label(format!("{r:.1}"));
                            ui.end_row();
                        }
                    });
                    ui.label(format!(
                        "Free volume V_f, half the buffer shell (fuel failure's definition): {:.3e} m³.",
                        s(1)
                    ));
                }
            }
            Rung::Decay => {
                egui::ComboBox::from_label("parent")
                    .selected_text(DECAY_NUCLIDES[c.decay_nuclide as usize].0)
                    .show_ui(ui, |ui| {
                        for (i, (n, _)) in DECAY_NUCLIDES.iter().enumerate() {
                            ui.selectable_value(&mut c.decay_nuclide, i as u8, *n);
                        }
                    });
                ui.add(egui::Slider::new(&mut c.t_over_half, 0.0..=5.0).text("time, parent half-lives"));
                if ui.button(if c.playing { "Pause" } else { "Play" }).clicked() {
                    c.playing = !c.playing;
                }
                ui.label(format!(
                    "Parent half-life (ENDF/B-VIII.0, in boon-lay): {}. Surviving parents now: {:.3}; the decay law gives {:.3} ± {:.3} (1σ for {} atoms).",
                    human_time(s(0)),
                    s(1),
                    s(3),
                    s(2),
                    crate::engine::DECAY_ATOMS
                ));
                ui.label("Each atom drew its own branch and its own exponential lifetimes, t = -T½ ln(ξ)/ln 2, from the crate's sampler. Positions are random in the kernel and carry no meaning here: decay does not move an atom.");
            }
            Rung::Walk => {
                ui.add(egui::Slider::new(&mut c.walk_temp_c, 1000.0..=1600.0).text("temperature, °C"));
                ui.add(
                    egui::Slider::new(&mut c.walk_dt_s, 60.0..=1.0e5)
                        .logarithmic(true)
                        .text("simulated s per frame"),
                );
                ui.horizontal(|ui| {
                    if ui.button(if c.walk_running { "Pause" } else { "Run" }).clicked() {
                        c.walk_running = !c.walk_running;
                    }
                    if ui.button("Restart").clicked() {
                        self.walk_reset = true;
                    }
                });
                ui.label(format!(
                    "CRP-6 Case 1: Cs-137 born uniformly in the 212.5 µm kernel, its surface a perfect sink, D = {:.3e} m²/s. t = {}: {:.3} of {} walkers have reached the surface; Crank's series gives {:.3}.",
                    s(3),
                    human_time(s(0)),
                    s(1),
                    s(4),
                    s(2)
                ));
                ui.label("Each walker hops the largest sphere that touches no interface (WoSWalker::hop) and adds that hop's sampled exit time to its own clock; there is no time step. 1200 °C is Case 1a (Crank 0.5337 at 200 h), 1600 °C Case 1b.");
                ui.label("Why not all five layers? A walker at an interface it rarely crosses is put back 20 nm away, so its hops add nanoseconds: one walker took 2·10⁷ steps to advance 221 s at 1200 °C (measured 2026-10-05). Release through the SiC takes days, so the five-layer walk cannot run live (#551); it has no release record either (see the lesson).");
                ui.separator();
                if ui.button("Run the CRP-6 Case 1 check").clicked() {
                    self.check_wanted = true;
                }
                if let Some(k) = &self.check {
                    ui.label(format!(
                        "Kernel only, absorbing surface: D = {:.3e} m²/s. Worst |Walk-on-Spheres − Crank| over 0-200 h: {:.4}, against a 1σ of {:.4}. Verification against an exact solution, not validation.",
                        k.scalars.first().copied().unwrap_or(f64::NAN),
                        k.scalars.get(1).copied().unwrap_or(f64::NAN),
                        k.scalars.get(2).copied().unwrap_or(f64::NAN)
                    ));
                }
            }
            Rung::Layers => {
                egui::ComboBox::from_label("nuclide")
                    .selected_text(LAYER_NUCLIDES[c.layer_nuclide as usize].0)
                    .show_ui(ui, |ui| {
                        for (i, (n, _)) in LAYER_NUCLIDES.iter().enumerate() {
                            ui.selectable_value(&mut c.layer_nuclide, i as u8, *n);
                        }
                    });
                ui.add(egui::Slider::new(&mut c.layer_temp_c, 800.0..=1800.0).text("temperature, °C"));
                ui.label(format!(
                    "At {:.0} °C: D_PyC = {:.3e}, D_SiC = {:.3e} m²/s, a contrast of {:.0}; a walker reaching the SiC from the PyC crosses with probability {:.2e} (K = 1).",
                    c.layer_temp_c,
                    s(0),
                    s(1),
                    s(0) / s(1),
                    s(2)
                ));
                if let Some(f) = f {
                    for n in &f.names {
                        ui.label(egui::RichText::new(n).color(Color32::from_rgb(255, 190, 120)));
                    }
                }
                ui.label("The buffer's 1e-8 m²/s is the code's stand-in for porous carbon, not a measured value. Fluence is zero (Jiang et al. 2023 correlations, as the walker calls them).");
            }
            Rung::Failure | Rung::Release => {
                ui.add(egui::Slider::new(&mut c.irr_c, 600.0..=1000.0).text("irradiation temperature, °C"));
                ui.add(egui::Slider::new(&mut c.hold_c, 1200.0..=2500.0).text("accident hold, °C"));
                ui.add(
                    egui::Slider::new(&mut c.hours, 10.0..=1000.0)
                        .logarithmic(true)
                        .text("hold, h"),
                );
                if self.rung == Rung::Failure {
                    c.cursor_h = c.cursor_h.min(c.hours);
                    ui.add(egui::Slider::new(&mut c.cursor_h, 0.0..=c.hours).text("now, h"));
                    if ui.button(if c.playing { "Pause" } else { "Play" }).clicked() {
                        c.playing = !c.playing;
                    }
                    ui.label(format!(
                        "At {:.0} h: φ₁ {:.2e}, φ₂ {:.2e}, in service {:.2e}; gas {:.1} MPa, SiC stress {:.0} MPa. End-of-irradiation φ₁: {:.2e}.",
                        s(0), s(1), s(2), s(3), s(4), s(5), s(7)
                    ));
                    ui.label(format!(
                        "The {POPULATION} particles above are a picture of the fraction (each fails when the fraction passes its own random draw), not {POPULATION} simulated particles."
                    ));
                    ui.label("HTR-10's particle with STAND-IN strength, Weibull modulus and fluence (EO 1607 / HTR-Module values, not HTR-10 data). The PANAMA-I equations were built for German TRISO at 1600-2500 °C; elsewhere this is an extrapolation. KORA oxidation failure is not modelled (#441); the Kugeler qualification band is unchecked (#383).");
                } else {
                    c.cursor_h = c.cursor_h.min(c.hours);
                    ui.add(egui::Slider::new(&mut c.cursor_h, 0.0..=c.hours).text("now, h (the diagram)"));
                    if ui.button(if c.playing { "Pause" } else { "Play" }).clicked() {
                        c.playing = !c.playing;
                    }
                    if let Some(f) = f {
                        let (n, _) = release_dims(f);
                        egui::ComboBox::from_label("show")
                            .selected_text(pick_label(f, c.release_pick))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut c.release_pick, PICK_ALL, pick_label(f, PICK_ALL));
                                for g in 0..GROUPS.len() {
                                    if f.tags.iter().any(|t| *t as usize == g) {
                                        let p = PICK_GROUP + g as u8;
                                        ui.selectable_value(&mut c.release_pick, p, pick_label(f, p));
                                    }
                                }
                                for i in 0..n {
                                    let p = PICK_ONE + i as u8;
                                    ui.selectable_value(&mut c.release_pick, p, pick_label(f, p));
                                }
                            });
                    }
                    ui.add(
                        egui::Slider::new(&mut c.f_hm, 1.0e-6..=1.0e-3)
                            .logarithmic(true)
                            .text("f_hm, heavy-metal contamination"),
                    );
                    ui.add(
                        egui::Slider::new(&mut c.k_plate, 1.0e-7..=1.0e-2)
                            .logarithmic(true)
                            .text("k_plate, 1/s (halogens, metals)"),
                    );
                    ui.add(
                        egui::Slider::new(&mut c.k_clean, 1.0e-7..=1.0e-2)
                            .logarithmic(true)
                            .text("k_clean, 1/s (noble gases, halogens)"),
                    );
                    ui.checkbox(&mut c.leak_on, "primary-circuit leak");
                    if c.leak_on {
                        ui.add(
                            egui::Slider::new(&mut c.k_leak, 1.0e-8..=1.0e-2)
                                .logarithmic(true)
                                .text("k_leak, 1/s"),
                        );
                    }
                    if ui.button("Reset to the reference case").clicked() {
                        let d = Controls::default();
                        c.f_hm = d.f_hm;
                        c.k_plate = d.k_plate;
                        c.k_clean = d.k_clean;
                        c.leak_on = d.leak_on;
                    }
                    if let Some(f) = f {
                        let nucs = selected_nuclides(f, c.release_pick);
                        let (_, p) = release_dims(f);
                        let inv: f64 = nucs.iter().map(|&i| release_header(f, i, 0)).sum();
                        let src = |pt: usize| nucs.iter().map(|&i| release_value(f, i, pt, rel::S)).sum::<f64>();
                        let half = match nucs.as_slice() {
                            [one] => format!("; half-life {}", human_time(std::f64::consts::LN_2 / release_header(f, *one, 1))),
                            _ => String::new(),
                        };
                        if p > 0 {
                            ui.label(format!(
                                "{}: core inventory {inv:.3e} Bq (Liu & Cao 2002 Table 1){half}. Source S into the helium: {:.3e} atoms/s at normal operation, {:.3e} at the end of the hold (in-service failure {:.2e}).",
                                pick_label(f, c.release_pick),
                                src(0),
                                src(p - 1),
                                f.series.first().and_then(|s| s.ys.last()).copied().unwrap_or(f64::NAN)
                            ));
                        }
                        let skipped: Vec<&str> = f.names.iter().filter_map(|n| n.strip_prefix(RELEASE_SKIPPED)).collect();
                        if !skipped.is_empty() {
                            ui.label(format!(
                                "In the inventory but not in TRISO-ATOPS's nuclide table, so not shown: {}.",
                                skipped.join(", ")
                            ));
                        }
                    }
                    ui.label("Each nuclide goes through TRISO-ATOPS's own chain: diffusion coefficients, <R/B>_fail (noble-gas fit for the volatiles, Booth diffusion for Cs and Sr, SiC breakthrough for Ag), the release rate, the matrix-graphite attenuation (metals are held up in the fuel element's graphite; volatiles pass straight through), then upstream's routing: noble gases do not plate out, and the purification system scrubs noble gases and halogens but not metals. No parent in-growth: I-135 decaying to Xe-135 is not followed (live_pools). Silver (Ag-110m) is zero at normal operation: in TRISO-ATOPS's breakthrough model it has not yet crossed intact SiC at the irradiation temperature (the membrane time lag); during a hot hold it does. During the hold this is TRISO-ATOPS's steady normal-operation release evaluated at the hold temperature, not its accident model (sembawang's, which depletes the kernel); the metals' Booth release is the one that differs most. Kernel radius 213 µm, SiC 35 µm, grain 10 µm and the 40-year run are the NP-MHTGR reference case, not HTR-10 values.");
                    ui.label(format!(
                        "The diagram: the fuel feeds the circulating helium at S; the helium loses atoms to plate-out (k_plate C), the purification system (k_clean C), a leak out of the circuit (k_leak C) and decay (λC); the plated and purified pools lose atoms only to decay. Arrow width and the number of dots show each flow on a log scale; the dots' speed is not physical. Playing moves \"now\" through the hold at {:.0} h per second.",
                        c.hours / 10.0
                    ));
                    ui.label("Defaults: f_hm from the NP-MHTGR case TRISO-ATOPS ships; k_plate and k_clean from Stoyer et al. 2026 Case A Table 3. The pools start from a 40-year run at the irradiation temperature (long-lived Cs-137 plate-out is still growing then, as in upstream's normal operation) and are carried through the hold exactly (live_pools, not a port). <R/B> uses the hold temperature. Air ingress releases exactly 0 Bq in this model (#446): it has no oxidation path.");
                }
            }
            Rung::Chemistry => {
                ui.add(egui::Slider::new(&mut c.o2_kpa, 0.1..=21.3).logarithmic(true).text("O₂, kPa"));
                ui.add(egui::Slider::new(&mut c.steam_kpa, 0.1..=100.0).logarithmic(true).text("steam, kPa"));
                ui.add(egui::Slider::new(&mut c.h2_kpa, 0.0..=2.0).text("H₂, kPa"));
                ui.label("Dotted: inside the range the fit was measured over. Dashed: extrapolated, which for a thick graphite body at high temperature OVER-states the rate (in-pore and boundary-layer diffusion take over). At core temperatures the oxygen supply, not the kinetics, limits air attack.");
                ui.label("Transcription checks only: the code reproduces its sources' equations. Missing: oxidation-driven particle failure (#441), air oxidation of kernels (#444), a hydrolysis model valid at high steam pressure (#418).");
            }
            Rung::SourceTerm => {
                ui.label(format!(
                    "Recorded, not computed here: {}.",
                    crate::recorded::SOURCE
                ));
                ui.label("What crosses the seam: released activity per nuclide over 96 h, the hand-off from this track to sembawang's source term and the dispersion track.");
                ui.hyperlink_to(
                    "Open the dispersion demo at its capstone",
                    "../dispersion/?rung=capstone",
                );
                ui.label("The gaps travel downstream: #446 (air ingress releases nothing here), #383 (qualification band unchecked), #296 (HTR-10 application).");
            }
        }
        ui.separator();
        if let Some(f) = f {
            ui.label(format!(
                "Last calculation: {:.1} ms in the background {}.",
                f.ms,
                if cfg!(target_arch = "wasm32") { "worker" } else { "thread" }
            ));
        }
        if let Some((_, t)) = self.in_flight {
            ui.label(format!("Computing ({:.1} s)…", now_s() - t));
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 16, 20));
        if self.rung.is_particle() {
            // The particle in the upper part, a small plot below it.
            let inset_h = if self.rung == Rung::Triso { 0.0 } else { (rect.height() * 0.32).max(150.0) };
            let top = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - inset_h));
            self.view.handle_input(ui, &resp);
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    self.tapped = self.region_at(top, p);
                }
            }
            self.draw_particle(&painter, top);
            if inset_h > 0.0 {
                let inset = Rect::from_min_max(Pos2::new(rect.left(), top.bottom()), rect.max);
                painter.rect_filled(inset, 0.0, Color32::from_rgb(20, 23, 28));
                let (series, spec) = match self.rung {
                    Rung::Walk if self.check.is_some() => (
                        &self.check.as_ref().unwrap().series,
                        Spec::lin("CRP-6 Case 1: release from the kernel, 0-200 h", "h", "released fraction"),
                    ),
                    Rung::Walk => (
                        self.frame.as_ref().map(|f| &f.series).unwrap_or(&EMPTY),
                        Spec::lin("Reached the kernel surface (CRP-6 Case 1)", "h", "fraction"),
                    ),
                    _ => (
                        self.frame.as_ref().map(|f| &f.series).unwrap_or(&EMPTY),
                        Spec::lin("Parents left", "parent half-lives", "fraction"),
                    ),
                };
                draw_panels(&painter, inset, series, &[spec], (self.plot_text * 0.85).max(9.0));
            }
        } else if self.rung == Rung::SourceTerm {
            // Below the always-visible button row.
            let below = Rect::from_min_max(rect.min + Vec2::new(0.0, BUTTON_ROW), rect.max);
            draw_source_term(&painter, below, self.plot_text);
        } else {
            let mut area = Rect::from_min_max(rect.min + Vec2::new(0.0, BUTTON_ROW), rect.max);
            if self.rung == Rung::Failure {
                let strip = (rect.height() * 0.28).clamp(110.0, 260.0);
                let r = Rect::from_min_max(area.min, Pos2::new(rect.right(), area.top() + strip));
                if let Some(f) = &self.frame {
                    draw_population(&painter, r, &f.tags, self.plot_text);
                }
                area = Rect::from_min_max(Pos2::new(rect.left(), r.bottom()), rect.max);
            }
            if self.rung == Rung::Release {
                // The living diagram on top, the plots (with the same "now")
                // below it.
                let strip = (rect.height() * 0.5).clamp(240.0, 460.0);
                let r = Rect::from_min_max(area.min, Pos2::new(rect.right(), area.top() + strip));
                if let Some(f) = &self.frame {
                    draw_pools(&painter, r, f, self.c.release_pick, self.c.cursor_h, now_s(), self.plot_text);
                }
                area = Rect::from_min_max(Pos2::new(rect.left(), r.bottom()), rect.max);
            }
            let specs = self.plot_specs();
            if let Some(f) = &self.frame {
                if self.rung == Rung::Release {
                    let series = release_series(f, self.c.release_pick);
                    draw_panels(&painter, area, &series, &specs, self.plot_text);
                } else {
                    draw_panels(&painter, area, &f.series, &specs, self.plot_text);
                }
            }
        }
        if let Some(e) = &self.error {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                e,
                egui::FontId::proportional(14.0),
                Color32::from_rgb(255, 130, 130),
            );
        } else if self.frame.is_none() && self.rung != Rung::SourceTerm {
            painter.text(
                rect.center() - Vec2::new(0.0, 60.0),
                egui::Align2::CENTER_CENTER,
                if self.rung == Rung::Decay {
                    "Reading the decay data in the background…"
                } else {
                    "Computing in the background…"
                },
                egui::FontId::proportional(15.0),
                Color32::LIGHT_GRAY,
            );
        }
        self.panel.reopen_button(ui, rect);
        if let Some(z) = zoom_buttons(ui, rect) {
            if self.rung.is_particle() {
                apply_zoom(&mut self.view, rect, z);
            } else {
                self.plot_text = match z {
                    Zoom::In => (self.plot_text * 1.2).min(28.0),
                    Zoom::Out => (self.plot_text / 1.2).max(8.0),
                    Zoom::Reset => 13.0,
                };
            }
        }
    }

    fn plot_specs(&self) -> Vec<Spec> {
        match self.rung {
            Rung::Layers => vec![Spec {
                title: "Diffusion coefficient in each layer (the walker's lookup)",
                xlabel: "°C",
                ylabel: "D, m²/s",
                xlog: false,
                ylog: true,
                cursor: Some(self.c.layer_temp_c),
            }],
            Rung::Failure => vec![
                Spec {
                    title: "Failure fractions during the hold (boon-lay fuel failure)",
                    xlabel: "h",
                    ylabel: "fraction",
                    xlog: false,
                    ylog: true,
                    cursor: Some(self.c.cursor_h),
                },
                Spec {
                    title: "Inside the particle",
                    xlabel: "h",
                    ylabel: "MPa",
                    xlog: false,
                    ylog: false,
                    cursor: Some(self.c.cursor_h),
                },
            ],
            Rung::Chemistry => vec![
                Spec {
                    title: "Graphite oxidation, fraction of the mass per second",
                    xlabel: "°C",
                    ylabel: "1/s",
                    xlog: false,
                    ylog: true,
                    cursor: None,
                },
                Spec {
                    title: "Steam on a bare kernel: stored noble gas released",
                    xlabel: "°C",
                    ylabel: "fraction",
                    xlog: false,
                    ylog: true,
                    cursor: None,
                },
            ],
            _ => vec![
                Spec {
                    title: "Into the helium and its three pools",
                    xlabel: "h of the hold",
                    ylabel: "atoms/s or Bq",
                    xlog: false,
                    ylog: true,
                    cursor: Some(self.c.cursor_h),
                },
                Spec {
                    title: "Leaked from the primary circuit",
                    xlabel: "h of the hold",
                    ylabel: "atoms",
                    xlog: false,
                    ylog: true,
                    cursor: Some(self.c.cursor_h),
                },
            ],
        }
    }

    /// The region under a tap, read from the computed slice (rung 1).
    fn region_at(&self, rect: Rect, p: Pos2) -> Option<String> {
        let f = self.frame.as_ref()?;
        let w = self.view.to_world(rect, p);
        let r = (w[0] * w[0] + w[1] * w[1]).sqrt();
        let k = f.radii.iter().position(|edge| r < *edge).unwrap_or(5);
        Some(format!("{} (r = {r:.0} µm)", REGIONS[k].1))
    }

    fn draw_particle(&self, painter: &egui::Painter, rect: Rect) {
        let painter = painter.with_clip_rect(rect);
        let v = &self.view;
        let Some(f) = &self.frame else { return };
        if self.rung == Rung::Triso && !f.regions.is_empty() {
            let n = f.cells as usize;
            let half = f.scalars[0];
            let step = 2.0 * half / n as f64;
            for row in 0..n {
                // Run-length: one rectangle per run of equal regions.
                let mut col = 0;
                while col < n {
                    let k = f.regions[row * n + col] as usize;
                    let mut end = col + 1;
                    while end < n && f.regions[row * n + end] as usize == k {
                        end += 1;
                    }
                    if k < 5 {
                        let a = v.to_screen(rect, -half + col as f64 * step, half - row as f64 * step);
                        let b = v.to_screen(rect, -half + end as f64 * step, half - (row + 1) as f64 * step);
                        // Half a pixel of overlap so rows leave no seams.
                        painter.rect_filled(Rect::from_two_pos(a, b).expand(0.5), 0.0, REGIONS[k].0);
                    }
                    col = end;
                }
            }
            let mut y = rect.top() + BUTTON_ROW + 4.0;
            for (c, name) in REGIONS.iter().take(5) {
                painter.rect_filled(Rect::from_min_size(Pos2::new(rect.left() + 12.0, y), Vec2::splat(12.0)), 2.0, *c);
                painter.text(Pos2::new(rect.left() + 30.0, y + 6.0), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(13.0), Color32::LIGHT_GRAY);
                y += 18.0;
            }
            if let Some(t) = &self.tapped {
                painter.text(Pos2::new(rect.left() + 12.0, y + 8.0), egui::Align2::LEFT_TOP, format!("tapped: {t}"), egui::FontId::proportional(13.0), Color32::WHITE);
            }
            painter.text(
                Pos2::new(rect.left() + 12.0, rect.bottom() - 34.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}, z = 0 slice, {n} × {n} region lookups", GEOMETRIES[self.c.geometry as usize].0),
                egui::FontId::proportional(12.0),
                Color32::LIGHT_GRAY,
            );
        } else {
            // Layer outlines, then the atoms.
            for (k, r) in f.radii.iter().enumerate().rev() {
                let c = v.to_screen(rect, 0.0, 0.0);
                painter.circle_filled(c, (*r * v.scale) as f32, REGIONS[k].0.gamma_multiply(0.35));
                painter.circle_stroke(c, (*r * v.scale) as f32, Stroke::new(1.0, REGIONS[k].0));
            }
            let rad = if self.rung == Rung::Decay { 3.0 } else { 2.2 };
            for (i, p) in f.dots.chunks(2).enumerate() {
                let colour = if self.rung == Rung::Decay {
                    SPECIES[(f.tags.get(i).copied().unwrap_or(0.0) as usize) % SPECIES.len()]
                } else {
                    Color32::from_rgb(255, 230, 120)
                };
                painter.circle_filled(v.to_screen(rect, p[0], p[1]), rad, colour);
            }
            if self.rung == Rung::Decay {
                let mut y = rect.top() + BUTTON_ROW + 4.0;
                for (k, name) in f.names.iter().enumerate() {
                    let count = f.tags.iter().filter(|t| **t as usize == k).count();
                    painter.circle_filled(Pos2::new(rect.left() + 18.0, y + 6.0), 5.0, SPECIES[k % SPECIES.len()]);
                    painter.text(Pos2::new(rect.left() + 30.0, y + 6.0), egui::Align2::LEFT_CENTER, format!("{name}: {count}"), egui::FontId::proportional(13.0), Color32::LIGHT_GRAY);
                    y += 18.0;
                }
            }
        }
        micrometre_scale_bar(&painter, rect, v);
    }
}

static EMPTY: Vec<Series> = Vec::new();

/// Height of the always-visible button row (Controls », +, −, Reset), which
/// nothing drawn on the main view may sit under.
const BUTTON_ROW: f32 = 52.0;

/// A plot panel's axes.
struct Spec {
    title: &'static str,
    xlabel: &'static str,
    ylabel: &'static str,
    xlog: bool,
    ylog: bool,
    /// A vertical line at this x (the slider's value).
    cursor: Option<f64>,
}

impl Spec {
    fn lin(title: &'static str, xlabel: &'static str, ylabel: &'static str) -> Self {
        Spec {
            title,
            xlabel,
            ylabel,
            xlog: false,
            ylog: false,
            cursor: None,
        }
    }
}

/// A scale bar in micrometres (the library's `scale_bar` labels its world
/// unit as cm, the Monte Carlo demo's).
fn micrometre_scale_bar(painter: &egui::Painter, rect: Rect, view: &View) {
    let bar = [10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0]
        .into_iter()
        .find(|&m| m * view.scale >= 80.0)
        .unwrap_or(1000.0);
    let x0 = rect.left() + 16.0;
    let y0 = rect.bottom() - 12.0;
    let x1 = x0 + (bar * view.scale) as f32;
    painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y0)], Stroke::new(2.0, Color32::WHITE));
    painter.text(
        Pos2::new(x1 + 6.0, y0),
        egui::Align2::LEFT_CENTER,
        format!("{bar} µm"),
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
}

/// Seconds as the most readable unit.
fn human_time(s: f64) -> String {
    if !s.is_finite() {
        return "—".into();
    }
    let (v, u) = if s < 120.0 {
        (s, "s")
    } else if s < 7200.0 {
        (s / 60.0, "min")
    } else if s < 2.0 * 86_400.0 {
        (s / 3600.0, "h")
    } else if s < 2.0 * 3.156e7 {
        (s / 86_400.0, "d")
    } else {
        (s / 3.156e7, "y")
    };
    format!("{v:.3} {u}")
}

/// The drawn population of rung 5: one small TRISO per particle, coloured by
/// what failed it.
fn draw_population(painter: &egui::Painter, rect: Rect, tags: &[f64], text: f32) {
    let side = (POPULATION as f64).sqrt().ceil() as usize;
    let cell = ((rect.width() - 24.0).min(rect.height() - 30.0) / side as f32).max(2.0);
    let x0 = rect.center().x - cell * side as f32 / 2.0;
    let y0 = rect.top() + 4.0;
    let mut counts = [0usize; 3];
    for (i, t) in tags.iter().enumerate() {
        let k = (*t as usize).min(2);
        counts[k] += 1;
        let c = Pos2::new(x0 + (i % side) as f32 * cell + cell / 2.0, y0 + (i / side) as f32 * cell + cell / 2.0);
        let colour = [Color32::from_rgb(46, 134, 193), Color32::from_rgb(255, 170, 80), Color32::from_rgb(200, 140, 255)][k];
        painter.circle_filled(c, cell * 0.42, colour);
        if k == 0 {
            painter.circle_filled(c, cell * 0.2, Color32::from_rgb(192, 57, 43));
        }
    }
    painter.text(
        Pos2::new(rect.left() + 12.0, y0 + cell * side as f32 + 4.0),
        egui::Align2::LEFT_TOP,
        format!("{} intact · {} burst (pressure vessel) · {} SiC decomposed", counts[0], counts[1], counts[2]),
        egui::FontId::proportional((text * 0.9).max(9.0)),
        Color32::LIGHT_GRAY,
    );
}

/// The release rung's selection: everything, one transport group, or one
/// nuclide (see `Controls::release_pick`).
const PICK_ALL: u8 = 0;
const PICK_GROUP: u8 = 1;
const PICK_ONE: u8 = 10;

/// Colours of TRISO-ATOPS's transport groups, in [`GROUPS`] order: the dots
/// of the diagram.
const GROUP_COLOURS: [Color32; 5] = [
    Color32::from_rgb(110, 210, 255),
    Color32::from_rgb(205, 140, 255),
    Color32::from_rgb(255, 170, 80),
    Color32::from_rgb(235, 235, 245),
    Color32::from_rgb(140, 230, 140),
];

/// The nuclides (frame indices) a pick selects.
fn selected_nuclides(f: &Frame, pick: u8) -> Vec<usize> {
    let (n, _) = release_dims(f);
    (0..n)
        .filter(|&i| match pick {
            PICK_ALL => true,
            p if p >= PICK_ONE => i == (p - PICK_ONE) as usize,
            p => f.tags.get(i).map(|t| *t as u8) == Some(p - PICK_GROUP),
        })
        .collect()
}

fn pick_label(f: &Frame, pick: u8) -> String {
    let (n, _) = release_dims(f);
    match pick {
        PICK_ALL => format!("all {n} nuclides"),
        p if p >= PICK_ONE => {
            let i = (p - PICK_ONE) as usize;
            match (f.names.get(i), f.tags.get(i)) {
                (Some(name), Some(t)) => format!("{name} ({})", GROUPS[*t as usize]),
                _ => "—".into(),
            }
        }
        p => format!("all {}", GROUPS[(p - PICK_GROUP) as usize]),
    }
}

/// `release_value` of nuclide `nuc` at `hour`, linear between the accident
/// steps (short against every curve's change), clamped to the ends.
fn at_cursor(f: &Frame, nuc: usize, k: usize, hour: f64) -> f64 {
    let Some(xs) = f.series.first().map(|s| &s.xs) else { return 0.0 };
    let v = |pt: usize| release_value(f, nuc, pt, k);
    match xs.iter().position(|&h| h >= hour) {
        None => v(xs.len() - 1),
        Some(0) => v(0),
        Some(i) => {
            let (h0, h1) = (xs[i - 1], xs[i]);
            let w = if h1 > h0 { (hour - h0) / (h1 - h0) } else { 1.0 };
            v(i - 1) + w * (v(i) - v(i - 1))
        }
    }
}

/// The plots for a pick: sums over its nuclides (pools as activity, Bq =
/// lambda x atoms; the source and the leak as atoms).
fn release_series(f: &Frame, pick: u8) -> Vec<Series> {
    let nucs = selected_nuclides(f, pick);
    let (_, p) = release_dims(f);
    let Some(xs) = f.series.first().map(|s| s.xs.clone()) else { return Vec::new() };
    let sum = |k: usize, bq: bool| -> Vec<f64> {
        (0..p)
            .map(|pt| {
                nucs.iter()
                    .map(|&i| release_value(f, i, pt, k) * if bq { release_header(f, i, 1) } else { 1.0 })
                    .sum()
            })
            .collect()
    };
    let line = |label: &str, colour: u8, panel: u8, ys: Vec<f64>| Series {
        label: label.into(),
        style: 1,
        colour,
        panel,
        xs: xs.clone(),
        ys,
    };
    vec![
        line("source into the helium S, atoms/s", 1, 0, sum(rel::S, false)),
        line("circulating activity, Bq", 0, 0, sum(rel::C, true)),
        line("plated out, Bq", 5, 0, sum(rel::P, true)),
        line("in the purification system, Bq", 2, 0, sum(rel::H, true)),
        line("held in the fuel elements' matrix graphite, Bq", 4, 0, sum(rel::G, true)),
        line("leaked from the circuit (cumulative atoms)", 3, 1, sum(rel::LEAKED, false)),
    ]
}

/// Where the boundary of a `half`-sized box centred at `c` is crossed by the
/// ray from `c` towards `to`.
fn box_edge(c: Pos2, half: Vec2, to: Pos2) -> Pos2 {
    let d = to - c;
    let t = (half.x / d.x.abs().max(1e-6)).min(half.y / d.y.abs().max(1e-6));
    c + d * t.min(1.0)
}

/// Spread `n` dots over the groups in proportion to `shares` (every group
/// with at least 2 % of the flow gets one), interleaved along the arrow.
fn dot_groups(shares: &[f64; 5], n: usize) -> Vec<usize> {
    let total: f64 = shares.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let counts: Vec<usize> = shares
        .iter()
        .map(|s| {
            let x = s / total;
            if x >= 0.02 { ((x * n as f64).round() as usize).max(1) } else { 0 }
        })
        .collect();
    let mut out = Vec::new();
    let most = counts.iter().copied().max().unwrap_or(0);
    for round in 0..most {
        for (g, c) in counts.iter().enumerate() {
            if round < *c {
                out.push(g);
            }
        }
    }
    out
}

/// Rung 7's living diagram, for every nuclide the pick selects: the fuel (a
/// TRISO particle in its fuel element's matrix graphite) feeds the
/// circulating helium, which loses atoms to plate-out, the purification
/// system, a leak out of the circuit and decay. Each pool shows its activity
/// at "now"; each flow is an arrow whose width and dot count grow with it
/// (log scale), its dots coloured by transport group in proportion to each
/// group's share of that flow. Every number is the engine's (`live_pools`'
/// pools, the terms of its balances, `base_activities`' hold-up); only the
/// dots' motion is illustration.
fn draw_pools(painter: &egui::Painter, rect: Rect, f: &Frame, pick: u8, now_h: f64, t: f64, text: f32) {
    let painter = painter.with_clip_rect(rect);
    let nucs = selected_nuclides(f, pick);
    if nucs.is_empty() {
        return;
    }
    let lam = |i: usize| release_header(f, i, 1);
    // Totals and per-group shares at "now".
    let total = |k: usize, bq: bool| -> f64 {
        nucs.iter().map(|&i| at_cursor(f, i, k, now_h) * if bq { lam(i) } else { 1.0 }).sum()
    };
    let shares = |k: usize| -> [f64; 5] {
        let mut g = [0.0; 5];
        for &i in &nucs {
            g[(f.tags[i] as usize).min(4)] += at_cursor(f, i, k, now_h);
        }
        g
    };
    let flows = [rel::S, rel::Q_PLATE, rel::Q_CLEAN, rel::Q_LEAK, rel::D_C, rel::D_P, rel::D_H];
    let q: Vec<f64> = flows.iter().map(|&k| total(k, false)).collect();
    let (c_bq, p_bq, h_bq, g_bq) = (total(rel::C, true), total(rel::P, true), total(rel::H, true), total(rel::G, true));
    let leaked = total(rel::LEAKED, false);
    let inventory: f64 = nucs.iter().map(|&i| release_header(f, i, 0)).sum();
    let any_rate = |k: usize| nucs.iter().any(|&i| release_header(f, i, k) > 0.0);
    // Log scale for widths and dots: six decades below the largest flow.
    let top = q.iter().cloned().fold(0.0f64, f64::max).max(1e-300).log10();
    let norm = |v: f64| if v > 0.0 { ((v.log10() - (top - 6.0)) / 6.0).clamp(0.0, 1.0) as f32 } else { 0.0 };

    let font = egui::FontId::proportional(text);
    let small = egui::FontId::proportional((text * 0.82).max(8.0));
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 21, 26));
    painter.text(
        rect.left_top() + Vec2::new(12.0, 8.0),
        egui::Align2::LEFT_TOP,
        format!("{} in the primary circuit at {now_h:.0} h of the hold", pick_label(f, pick)),
        font.clone(),
        Color32::WHITE,
    );
    // Group legend, the groups present.
    let mut x = rect.left() + 12.0;
    let ly = rect.top() + 14.0 + text * 1.5;
    for g in 0..5 {
        if nucs.iter().any(|&i| f.tags[i] as usize == g) {
            painter.circle_filled(Pos2::new(x + 5.0, ly), 4.5, GROUP_COLOURS[g]);
            let gl = painter.layout_no_wrap(GROUPS[g].to_string(), small.clone(), Color32::LIGHT_GRAY);
            let w = gl.size().x;
            painter.galley(Pos2::new(x + 13.0, ly - gl.size().y / 2.0), gl, Color32::LIGHT_GRAY);
            x += w + 26.0;
        }
    }
    let narrow = rect.width() < 600.0;
    let body = Rect::from_min_max(
        rect.min + Vec2::new(0.0, 22.0 + text * 2.6),
        rect.max - Vec2::new(0.0, if narrow { text * 3.2 } else { text * 1.6 }),
    );
    // Positions (fractions of the body): left to right when wide, top to
    // bottom on a phone.
    let wide = body.width() > body.height() * 1.5;
    let at = |x: f32, y: f32| Pos2::new(body.left() + x * body.width(), body.top() + y * body.height());
    let (fuel, he, plate, hps, leak) = if wide {
        (at(0.12, 0.5), at(0.45, 0.5), at(0.85, 0.17), at(0.85, 0.5), at(0.85, 0.83))
    } else {
        (at(0.5, 0.12), at(0.5, 0.5), at(0.17, 0.87), at(0.5, 0.87), at(0.83, 0.87))
    };
    let bw = if wide { (body.width() * 0.2).min(230.0) } else { (body.width() * 0.31).min(200.0) };
    let half = Vec2::new(bw / 2.0, (text * 2.4).max(26.0));
    // The fuel element: a disc of matrix graphite with one TRISO particle.
    let pebble = if wide { (body.height() * 0.36).min(body.width() * 0.1) } else { (body.height() * 0.115).min(body.width() * 0.2) };
    let fuel_half = Vec2::splat(pebble);
    let colours = [Color32::from_rgb(192, 57, 43), PALETTE[0], PALETTE[5], PALETTE[2], PALETTE[3]];

    // Arrows first, so the boxes sit on top of their ends. `along`: where on
    // the arrow its label sits (the fan-out is crowded on a phone).
    let arrow = |from: Pos2, fh: Vec2, to: Pos2, v: f64, sh: [f64; 5], colour: Color32, label: String, phase: f64, along: f32| {
        let (a, b) = (box_edge(from, fh, to), box_edge(to, half, from));
        let w = norm(v);
        if v <= 0.0 {
            painter.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(1.0, Color32::from_gray(80)), 6.0, 5.0));
        } else {
            painter.line_segment([a, b], Stroke::new(1.5 + 5.0 * w, colour.gamma_multiply(0.35)));
            let dir = (b - a).normalized();
            let tip = b - dir * 2.0;
            let side = Vec2::new(-dir.y, dir.x) * (5.0 + 3.0 * w);
            painter.add(egui::Shape::convex_polygon(
                vec![tip, tip - dir * (10.0 + 4.0 * w) + side, tip - dir * (10.0 + 4.0 * w) - side],
                colour,
                Stroke::NONE,
            ));
            let groups = dot_groups(&sh, 1 + (w * 13.0).round() as usize);
            let n = groups.len().max(1);
            for (k, g) in groups.iter().enumerate() {
                let s = ((t * 0.35 + phase + k as f64 / n as f64) % 1.0) as f32;
                painter.circle_filled(a + (b - a) * s, 2.2 + 1.3 * w, GROUP_COLOURS[*g]);
            }
        }
        let mid = a + (b - a) * along;
        let g = painter.layout(label, small.clone(), if v > 0.0 { Color32::LIGHT_GRAY } else { Color32::GRAY }, bw);
        let r = Rect::from_center_size(mid + Vec2::new(0.0, -10.0), g.size()).expand(2.0);
        painter.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(18, 21, 26, 220));
        painter.galley(r.min + Vec2::splat(2.0), g, Color32::WHITE);
    };
    let per_s = |v: f64| format!("{v:.2e} /s");
    let side = if wide { 0.5 } else { 0.7 };
    arrow(fuel, fuel_half, he, q[0], shares(rel::S), colours[0], format!("S {}", per_s(q[0])), 0.0, 0.5);
    arrow(
        he, half, plate, q[1], shares(rel::Q_PLATE), colours[2],
        if any_rate(2) { format!("plate-out {}", per_s(q[1])) } else { "no plate-out (noble gases)".into() },
        0.13, side,
    );
    arrow(
        he, half, hps, q[2], shares(rel::Q_CLEAN), colours[3],
        if any_rate(3) { format!("clean-up {}", per_s(q[2])) } else { "no clean-up (the HPS does not scrub metals)".into() },
        0.37, if wide { 0.5 } else { 0.4 },
    );
    arrow(
        he, half, leak, q[3], shares(rel::Q_LEAK), colours[4],
        if any_rate(4) { format!("leak {}", per_s(q[3])) } else { "leak off".into() },
        0.61, side,
    );

    // Decay: dots rising out of each pool and fading.
    let decay = |c: Pos2, v: f64, phase: f64| {
        let w = norm(v);
        if v <= 0.0 {
            return;
        }
        let dots = 1 + (w * 4.0).round() as usize;
        for k in 0..dots {
            let s = ((t * 0.25 + phase + k as f64 / dots as f64) % 1.0) as f32;
            let x = c.x + half.x * (0.85 - 0.4 * k as f32 / dots as f32);
            let p = Pos2::new(x, c.y - half.y - 4.0 - s * 22.0);
            painter.circle_filled(p, 2.0, Color32::from_white_alpha(((1.0 - s) * 200.0) as u8));
        }
    };
    decay(he, q[4], 0.0);
    decay(plate, q[5], 0.3);
    decay(hps, q[6], 0.6);

    // The fuel element and its particle. The SiC is drawn cracked: only a
    // failed or contaminated particle (or, for silver, diffusion through
    // intact SiC) releases. Atoms leave the kernel through the crack, cross
    // the matrix graphite (where the metals are held up) and enter the
    // helium.
    let toward = (box_edge(he, half, fuel) - fuel).normalized();
    painter.circle_filled(fuel, pebble, Color32::from_rgb(52, 56, 62));
    painter.circle_stroke(fuel, pebble, Stroke::new(1.5, colours[0]));
    let rp = pebble * 0.55;
    // HTR-10 particle proportions (radii 250, 340, 380, 415, 455 µm).
    let radii = [250.0f32, 340.0, 380.0, 415.0, 455.0].map(|r| rp * r / 455.0);
    for k in (0..5).rev() {
        painter.circle_filled(fuel, radii[k], REGIONS[k].0);
    }
    let crack_angle = toward.y.atan2(toward.x);
    for d in [-0.18f32, -0.06, 0.06, 0.18] {
        let a = crack_angle + d;
        let u = Vec2::new(a.cos(), a.sin());
        painter.line_segment([fuel + u * radii[2], fuel + u * radii[4]], Stroke::new(1.6, Color32::from_rgb(52, 56, 62)));
    }
    let src = shares(rel::S);
    let groups = dot_groups(&src, 3 + (norm(q[0]) * 9.0).round() as usize);
    let n = groups.len().max(1);
    for (k, g) in groups.iter().enumerate() {
        let s = ((t * 0.3 + k as f64 / n as f64) % 1.0) as f32;
        // A small fan around the crack, from the kernel to the pebble's rim.
        let a = crack_angle + ((k as f32 * 2.399).sin()) * 0.25;
        let u = Vec2::new(a.cos(), a.sin());
        painter.circle_filled(fuel + u * (radii[0] * 0.3 + s * (pebble - radii[0] * 0.3)), 2.2, GROUP_COLOURS[*g]);
    }
    let label_at = if wide { fuel + Vec2::new(0.0, pebble + text * 0.9) } else { fuel + Vec2::new(-pebble - 6.0, 0.0) };
    let align = if wide { egui::Align2::CENTER_TOP } else { egui::Align2::RIGHT_CENTER };
    let fuel_text = if g_bq > 0.0 {
        format!("fuel element\ncore {inventory:.2e} Bq\nmatrix graphite {g_bq:.2e} Bq")
    } else {
        format!("fuel element\ncore {inventory:.2e} Bq")
    };
    let gl = painter.layout(fuel_text, small.clone(), Color32::LIGHT_GRAY, if wide { bw * 1.3 } else { body.width() * 0.3 });
    let r = align.anchor_size(label_at, gl.size());
    painter.galley(r.min, gl, Color32::LIGHT_GRAY);

    // The pools: name, content at "now", and a gauge of its decade.
    let pools_bq = [c_bq, p_bq, h_bq];
    let top_bq = pools_bq.iter().cloned().fold(0.0f64, f64::max).max(1e-300).log10();
    let boxed = |c: Pos2, colour: Color32, name: &str, value: String, gauge: Option<f64>| {
        let r = Rect::from_center_size(c, half * 2.0);
        painter.rect_filled(r, 6.0, Color32::from_rgb(28, 32, 40));
        if let Some(bq) = gauge {
            let g = if bq > 0.0 { ((bq.log10() - (top_bq - 8.0)) / 8.0).clamp(0.0, 1.0) as f32 } else { 0.0 };
            let fill = Rect::from_min_max(Pos2::new(r.left(), r.bottom() - r.height() * g), r.max);
            painter.rect_filled(fill, 6.0, colour.gamma_multiply(0.35));
        }
        painter.rect_stroke(r, 6.0, Stroke::new(1.5, colour), egui::StrokeKind::Inside);
        painter.text(c - Vec2::new(0.0, text * 0.6), egui::Align2::CENTER_CENTER, name, font.clone(), Color32::WHITE);
        painter.text(c + Vec2::new(0.0, text * 0.75), egui::Align2::CENTER_CENTER, value, small.clone(), Color32::LIGHT_GRAY);
    };
    boxed(he, colours[1], "circulating helium", format!("{c_bq:.2e} Bq"), Some(c_bq));
    boxed(plate, colours[2], "plated out", format!("{p_bq:.2e} Bq"), Some(p_bq));
    boxed(hps, colours[3], "purification (HPS)", format!("{h_bq:.2e} Bq"), Some(h_bq));
    boxed(leak, colours[4], "left the circuit", format!("{leaked:.2e} atoms"), None);

    let note = painter.layout(
        "Dots: colour = transport group, number = share of the flow. Box fill: activity, 8 decades. Arrow width: the flow, 6 decades (log). Rising dots: decay. Dot speed is not physical; the particle is HTR-10's, drawn cracked.".into(),
        small,
        Color32::GRAY,
        rect.width() - 24.0,
    );
    painter.galley(Pos2::new(rect.left() + 12.0, rect.bottom() - 4.0 - note.size().y), note, Color32::GRAY);
}

/// Rung 8: the recorded release, one bar per nuclide (log scale).
fn draw_source_term(painter: &egui::Painter, rect: Rect, text: f32) {
    let font = egui::FontId::proportional(text);
    painter.text(rect.left_top() + Vec2::new(12.0, 14.0), egui::Align2::LEFT_TOP, "Released over 96 h, Bq (recorded, bounding)", font.clone(), Color32::WHITE);
    let rows = crate::recorded::RELEASE;
    let top = rect.top() + 60.0;
    let h = ((rect.height() - 140.0) / rows.len() as f32).clamp(20.0, 48.0);
    let label_w = text * 5.5;
    let w = rect.width() - label_w - 40.0;
    let (lo, hi) = (10.0f64, 13.0f64);
    for (i, (n, bq, core)) in rows.iter().enumerate() {
        let y = top + i as f32 * h;
        painter.text(Pos2::new(rect.left() + 12.0, y + h / 2.0), egui::Align2::LEFT_CENTER, *n, font.clone(), Color32::LIGHT_GRAY);
        let frac = ((bq.log10() - lo) / (hi - lo)).clamp(0.02, 1.0) as f32;
        let bar = Rect::from_min_size(Pos2::new(rect.left() + label_w, y + h * 0.15), Vec2::new(w * frac, h * 0.7));
        painter.rect_filled(bar, 2.0, PALETTE[0]);
        let label = painter.layout_no_wrap(
            format!("{bq:.2e}  ({:.0e} of core)", bq / core),
            egui::FontId::proportional((text * 0.85).max(9.0)),
            Color32::BLACK,
        );
        // Inside the bar when it fits, after it (in white) when not.
        let (at, colour) = if label.size().x + 12.0 < bar.width() {
            (Pos2::new(bar.left() + 6.0, bar.center().y - label.size().y / 2.0), Color32::BLACK)
        } else {
            (Pos2::new(bar.right() + 6.0, bar.center().y - label.size().y / 2.0), Color32::LIGHT_GRAY)
        };
        painter.galley_with_override_text_color(at, label, colour);
    }
    let y = top + rows.len() as f32 * h + 12.0;
    painter.text(
        Pos2::new(rect.left() + 12.0, y),
        egui::Align2::LEFT_TOP,
        format!("Total {:.3e} Bq. Bar length: log scale, 1e10 to 1e13 Bq.", crate::recorded::RELEASE_TOTAL_BQ),
        egui::FontId::proportional((text * 0.85).max(9.0)),
        Color32::GRAY,
    );
}

/// Stacked plots sharing one legend at the bottom. Ours dotted, references
/// and extrapolations dashed, recorded or sampled points as markers (the
/// workspace's plotting convention).
fn draw_panels(painter: &egui::Painter, rect: Rect, series: &[Series], specs: &[Spec], text: f32) {
    let small = egui::FontId::proportional((text * 0.85).max(8.0));
    let font = egui::FontId::proportional(text);
    // The legend: one line per label, measured first so the plots fit above.
    let mut seen: Vec<(&str, u8, u8)> = Vec::new();
    let legend: Vec<_> = series
        .iter()
        .filter(|s| {
            let key = (s.label.as_str(), s.style, s.colour);
            let new = !seen.contains(&key);
            if new {
                seen.push(key);
            }
            new
        })
        .map(|s| {
            let colour = PALETTE[(s.colour as usize) % PALETTE.len()];
            let style = ["solid", "dotted", "dashed", "markers"][(s.style as usize).min(3)];
            painter.layout(format!("{style}: {}", s.label), small.clone(), colour, rect.width() - 24.0)
        })
        .collect();
    let legend_h: f32 = legend.iter().map(|g| g.size().y + 3.0).sum::<f32>() + 6.0;
    let plots_h = rect.height() - legend_h;
    let n = specs.len().max(1);
    let each = plots_h / n as f32;
    for (k, spec) in specs.iter().enumerate() {
        let r = Rect::from_min_size(rect.min + Vec2::new(0.0, each * k as f32), Vec2::new(rect.width(), each));
        let panel_series: Vec<&Series> = series.iter().filter(|s| s.panel as usize == k).collect();
        draw_one(painter, r, &panel_series, spec, &font, &small);
    }
    let mut y = rect.top() + plots_h + 4.0;
    for g in legend {
        let h = g.size().y;
        painter.galley(Pos2::new(rect.left() + 12.0, y), g, Color32::WHITE);
        y += h + 3.0;
    }
}

fn draw_one(painter: &egui::Painter, rect: Rect, series: &[&Series], spec: &Spec, font: &egui::FontId, small: &egui::FontId) {
    painter.text(rect.left_top() + Vec2::new(12.0, 8.0), egui::Align2::LEFT_TOP, spec.title, font.clone(), Color32::WHITE);
    let plot = Rect::from_min_max(
        rect.left_top() + Vec2::new(font.size * 4.2, 32.0 + font.size),
        rect.right_bottom() - Vec2::new(16.0, font.size * 2.4),
    );
    if plot.width() < 40.0 || plot.height() < 30.0 {
        return;
    }
    let tx = |x: f64| if spec.xlog { x.log10() } else { x };
    let ty = |y: f64| if spec.ylog { y.log10() } else { y };
    let ok = |x: f64, y: f64| x.is_finite() && y.is_finite() && (!spec.xlog || x > 0.0) && (!spec.ylog || y > 0.0);
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for s in series {
        for (x, y) in s.xs.iter().zip(&s.ys) {
            if ok(*x, *y) {
                x0 = x0.min(tx(*x));
                x1 = x1.max(tx(*x));
                y0 = y0.min(ty(*y));
                y1 = y1.max(ty(*y));
            }
        }
    }
    if !(x0 < x1) {
        painter.text(plot.center(), egui::Align2::CENTER_CENTER, "nothing to plot (all zero)", small.clone(), Color32::GRAY);
        return;
    }
    if spec.ylog {
        y1 = y1.ceil();
        y0 = y0.floor().max(y1 - 12.0);
        if y0 >= y1 {
            y0 = y1 - 1.0;
        }
    } else {
        y0 = y0.min(0.0);
        if y1 <= y0 {
            y1 = y0 + 1.0;
        }
        y1 *= 1.05;
    }
    let to = |x: f64, y: f64| {
        Pos2::new(
            plot.left() + ((tx(x) - x0) / (x1 - x0)) as f32 * plot.width(),
            plot.bottom() - ((ty(y).max(y0) - y0) / (y1 - y0)) as f32 * plot.height(),
        )
    };
    painter.rect_stroke(plot, 0.0, Stroke::new(1.0, Color32::from_gray(90)), egui::StrokeKind::Inside);
    // Y grid.
    let yticks: Vec<f64> = if spec.ylog {
        let step = (((y1 - y0) / 6.0).ceil()).max(1.0) as i32;
        ((y0 as i32)..=(y1 as i32)).step_by(step as usize).map(|d| d as f64).collect()
    } else {
        nice_ticks(y0, y1)
    };
    for t in yticks {
        let py = plot.bottom() - ((t - y0) / (y1 - y0)) as f32 * plot.height();
        painter.line_segment([Pos2::new(plot.left(), py), Pos2::new(plot.right(), py)], Stroke::new(0.5, Color32::from_gray(50)));
        let label = if spec.ylog { format!("1e{t}") } else { short(t) };
        painter.text(Pos2::new(plot.left() - 4.0, py), egui::Align2::RIGHT_CENTER, label, small.clone(), Color32::GRAY);
    }
    let xticks: Vec<f64> = if spec.xlog {
        ((x0.ceil() as i32)..=(x1.floor() as i32)).map(|d| d as f64).collect()
    } else {
        nice_ticks(x0, x1)
    };
    for t in xticks {
        let px = plot.left() + ((t - x0) / (x1 - x0)) as f32 * plot.width();
        painter.line_segment([Pos2::new(px, plot.top()), Pos2::new(px, plot.bottom())], Stroke::new(0.5, Color32::from_gray(50)));
        let label = if spec.xlog { format!("1e{t}") } else { short(t) };
        painter.text(Pos2::new(px, plot.bottom() + 3.0), egui::Align2::CENTER_TOP, label, small.clone(), Color32::GRAY);
    }
    painter.text(Pos2::new(plot.right(), plot.bottom() + 3.0 + small.size * 1.2), egui::Align2::RIGHT_TOP, spec.xlabel, small.clone(), Color32::GRAY);
    painter.text(Pos2::new(plot.left(), plot.top() - 3.0), egui::Align2::LEFT_BOTTOM, spec.ylabel, small.clone(), Color32::GRAY);
    let clip = painter.with_clip_rect(plot.expand(4.0));
    for s in series {
        // A point below the lowest decade is left out, not pinned to the
        // floor (a pinned point reads as a value).
        let line: Vec<Pos2> = s
            .xs
            .iter()
            .zip(&s.ys)
            .filter(|(x, y)| ok(**x, **y) && ty(**y) >= y0)
            .map(|(x, y)| to(*x, *y))
            .collect();
        let colour = PALETTE[(s.colour as usize) % PALETTE.len()];
        match s.style {
            0 => {
                clip.add(egui::Shape::line(line, Stroke::new(3.0, colour)));
            }
            1 => clip.extend(egui::Shape::dotted_line(&line, colour, 5.0, 1.4)),
            2 => clip.extend(egui::Shape::dashed_line(&line, Stroke::new(1.5, colour), 8.0, 5.0)),
            _ => {
                for p in line {
                    clip.circle_filled(p, 3.5, colour);
                }
            }
        }
    }
    if let Some(cx) = spec.cursor {
        if ok(cx, if spec.ylog { 10f64.powf(y1) } else { y1 }) {
            let px = to(cx, if spec.ylog { 10f64.powf(y0) } else { y0 }).x;
            if px >= plot.left() && px <= plot.right() {
                painter.line_segment([Pos2::new(px, plot.top()), Pos2::new(px, plot.bottom())], Stroke::new(1.0, Color32::from_rgb(255, 230, 120)));
            }
        }
    }
}

/// About five round ticks over [a, b].
fn nice_ticks(a: f64, b: f64) -> Vec<f64> {
    let span = (b - a).abs().max(1e-300);
    let raw = span / 5.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let mut t = (a / step).ceil() * step;
    let mut out = Vec::new();
    while t <= b + 1e-9 * step && out.len() < 12 {
        out.push(t);
        t += step;
    }
    out
}

fn short(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else if v.abs() >= 1e4 || v.abs() < 1e-2 {
        format!("{v:.0e}")
    } else if v.fract().abs() < 1e-9 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}")
    }
}

impl eframe::App for TrisoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(link) = &self.link {
            let events = link.drain();
            self.handle(events);
        }
        self.play();
        self.pump();
        let animating = (self.rung == Rung::Walk && self.c.walk_running)
            || (matches!(self.rung, Rung::Decay | Rung::Failure) && self.c.playing)
            // The pool diagram's dots flow even while the cursor is paused.
            || self.rung == Rung::Release;
        if animating || self.in_flight.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        let title = format!("TRISO-ATOPS demo · {}", self.rung.title());
        if title != self.title {
            set_title(&ctx, &title);
            self.title = title;
        }
        let mut panel = std::mem::take(&mut self.panel);
        panel.show(ui, "TRISO-ATOPS", |ui| self.side_panel(ui));
        self.panel = panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}
