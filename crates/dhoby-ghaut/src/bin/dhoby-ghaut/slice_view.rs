//! The main view: a slice of the ASSEMBLED geometry, drawn from the solver's
//! own cell lookups (the crate's drawing HARD RULE), pannable and zoomable.
//!
//! The picture is re-rendered for whatever window the reader is looking at,
//! at screen resolution, so zooming into a pebble shows the pebble's TRISO
//! particles rather than blown-up pixels. ~~Rendering runs on the geometry
//! engine thread~~ **CHANGED 2026-10-05 (gh:#587):** a slice is one material
//! lookup per pixel **on the GPU** (`crate::gpu_view`), redrawn every frame
//! the view moves; the CPU slice on the geometry engine thread, a moment
//! after the reader stops moving, is the fallback (see `view3d`). Until a new
//! slice arrives the previous one stays on screen, placed in world
//! coordinates, so panning never waits. PNG exports are always the CPU's.

use dhoby_ghaut::web_demo::view::{apply_zoom, scale_bar_sized, zoom_buttons_sized, View};
use egui::{Color32, Pos2, Rect, RichText, TextureHandle, TextureOptions, Vec2};
use outram_blender::csg::gpu::render::GpuPlot;
use outram_mc_libs::geometry::plot::{
    ColourScheme, ImageData, PlotBasis, PlotColourBy, Rgb, SlicePlot, DEFAULT_PLOTTER_SEED,
};
use outram_mc_libs::geometry::position::Position;

use crate::engine::{palette, Ev, Req};
use crate::gpu_view::{Gpu, GpuSlot};

/// A named view of the review gate's minimum set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub name: &'static str,
    pub basis: PlotBasis,
    /// Position along the slice normal \[cm\].
    pub depth: f64,
    /// Centre of the view in the slice plane \[cm\].
    pub centre: [f64; 2],
    /// Half-width shown \[cm\].
    pub half_extent: f64,
}

pub struct SliceView {
    pub basis: PlotBasis,
    /// Position along the normal \[cm\] (y for X-Z, z for X-Y, x for Y-Z).
    pub depth: f64,
    pub view: View,
    texture: Option<(TextureHandle, [f64; 2], [f64; 2])>,
    /// Colours present in the shown slice, as legend rows.
    pub legend: Vec<(Color32, &'static str)>,
    next_id: u64,
    pending: Option<u64>,
    /// When the view or slice last changed (s), and whether a render is owed.
    changed_at: f64,
    dirty: bool,
    last_key: (PlotBasis, i64, i64, i64, i64, i64, i64),
    /// The preset this view was last set to, if the reader has not moved off it.
    pub at_preset: Option<&'static str>,
    /// Size of the last canvas, for exports.
    pub canvas: Vec2,
    /// The GPU tracer (gh:#587), shared with the 3D view; `None` draws on
    /// the CPU.
    pub gpu: Option<Gpu>,
    slot: GpuSlot,
    /// Where the GPU picture sits: centre and width in the plane \[cm\].
    gpu_place: Option<([f64; 2], [f64; 2])>,
}

impl SliceView {
    pub fn new() -> Self {
        Self {
            basis: PlotBasis::Xz,
            depth: 0.0,
            view: View::new(320.0),
            texture: None,
            legend: Vec::new(),
            next_id: 1,
            pending: None,
            changed_at: 0.0,
            dirty: true,
            last_key: (PlotBasis::Xz, 0, 0, 0, 0, 0, 0),
            at_preset: None,
            canvas: Vec2::new(800.0, 600.0),
            gpu: None,
            slot: GpuSlot::new(),
            gpu_place: None,
        }
    }

    /// Jump to a named view.
    pub fn go(&mut self, p: &Preset, rect: Rect) {
        self.basis = p.basis;
        self.depth = p.depth;
        self.view.half_extent = p.half_extent;
        self.view.fit(rect);
        self.view.centre = p.centre;
        self.view.touched = true;
        self.at_preset = Some(p.name);
        self.dirty = true;
    }

    /// Forget the picture (the geometry changed).
    pub fn invalidate(&mut self) {
        self.texture = None;
        self.slot.clear();
        self.gpu_place = None;
        self.legend.clear();
        self.dirty = true;
    }

    /// The slice origin for a view centred at `c` in the plane.
    pub fn origin(basis: PlotBasis, depth: f64, c: [f64; 2]) -> [f64; 3] {
        match basis {
            PlotBasis::Xy => [c[0], c[1], depth],
            PlotBasis::Xz => [c[0], depth, c[1]],
            PlotBasis::Yz => [depth, c[0], c[1]],
        }
    }

    /// The window currently on screen: origin, world width and pixels.
    pub fn window(&self, rect: Rect, max_px: f32) -> ([f64; 3], [f64; 2], [usize; 2]) {
        let w = [
            rect.width() as f64 / self.view.scale,
            rect.height() as f64 / self.view.scale,
        ];
        let k = (max_px / rect.width().max(rect.height())).min(1.0);
        let px = [
            ((rect.width() * k) as usize).max(16),
            ((rect.height() * k) as usize).max(16),
        ];
        (
            Self::origin(self.basis, self.depth, self.view.centre),
            w,
            px,
        )
    }

    /// Handle a slice the engine sent back.
    pub fn on_event(&mut self, ctx: &egui::Context, ev: &Ev) -> bool {
        if let Ev::Slice {
            id,
            basis,
            origin,
            width,
            image,
        } = ev
        {
            if Some(*id) != self.pending {
                return false;
            }
            self.pending = None;
            let c = match basis {
                PlotBasis::Xy => [origin[0], origin[1]],
                PlotBasis::Xz => [origin[0], origin[2]],
                PlotBasis::Yz => [origin[1], origin[2]],
            };
            let (tex, legend) = upload(ctx, image);
            self.texture = Some((tex, c, *width));
            self.legend = legend;
            self.gpu_place = None;
            return true;
        }
        false
    }

    /// Draw the view in `ui`'s remaining space and ask for a new render when
    /// the picture is out of date. `send` posts to the geometry engine.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        now: f64,
        have_geometry: bool,
        send: &mut impl FnMut(Req),
    ) -> Rect {
        let (rect, resp) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        self.canvas = rect.size();
        let before = (self.view.centre, self.view.scale);
        self.view.handle_input(ui, &resp);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(235));
        let gpu_tex = self.gpu.as_ref().and_then(|g| self.slot.texture(g));
        let picture = match (gpu_tex, self.gpu_place) {
            (Some(t), Some((c, w))) => Some((t, c, w)),
            _ => self.texture.as_ref().map(|(t, c, w)| (t.id(), *c, *w)),
        };
        if let Some((tex, c, w)) = picture {
            let a = self
                .view
                .to_screen(rect, c[0] - 0.5 * w[0], c[1] + 0.5 * w[1]);
            let b = self
                .view
                .to_screen(rect, c[0] + 0.5 * w[0], c[1] - 0.5 * w[1]);
            painter.image(
                tex,
                Rect::from_min_max(a, b),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if let Some(ids) = self.slot.take_ids() {
            self.legend = legend_from_ids(&ids);
        }
        if !have_geometry {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Assembling the geometry…",
                crate::style::Text::Subheading.font(),
                Color32::DARK_GRAY,
            );
        }
        scale_bar_sized(&painter, rect, &self.view, crate::style::Text::Small.size());
        let label = format!(
            "{} slice at {} = {:.1} cm{}",
            self.basis.name().to_uppercase(),
            match self.basis {
                PlotBasis::Xy => "z",
                PlotBasis::Xz => "y",
                PlotBasis::Yz => "x",
            },
            self.depth,
            if self.pending.is_some() {
                "   (rendering…)"
            } else {
                ""
            }
        );
        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -30.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            crate::style::Text::Body.font(),
            Color32::BLACK,
        );
        if let Some(z) = zoom_buttons_sized(ui, rect, crate::style::Text::Subheading.size()) {
            apply_zoom(&mut self.view, rect, z);
        }
        if (self.view.centre, self.view.scale) != before {
            self.at_preset = None;
        }
        // Re-render when the window changed and has been still for 0.3 s.
        let q = |x: f64| (x * 100.0).round() as i64;
        let key = (
            self.basis,
            q(self.depth),
            q(self.view.centre[0]),
            q(self.view.centre[1]),
            q(self.view.scale * 1000.0),
            rect.width() as i64,
            rect.height() as i64,
        );
        if key != self.last_key {
            self.last_key = key;
            self.changed_at = now;
            self.dirty = true;
        }
        let gpu_ready = have_geometry && self.gpu.as_ref().is_some_and(Gpu::poll);
        // The GPU became ready after a CPU picture (or has a new geometry):
        // trace it there, without waiting for the reader to move the view.
        if gpu_ready && gpu_tex.is_none() {
            self.dirty = true;
        }
        if gpu_ready {
            // GPU: every change, at screen resolution, once the last frame
            // has run (no frame is queued behind a slow one).
            if self.dirty && !self.slot.busy() {
                let ppp = ui.ctx().pixels_per_point();
                let (origin, width, pixels) =
                    self.window(rect, rect.width().max(rect.height()) * ppp);
                if let Some(g) = &self.gpu {
                    let plot = SlicePlot::new(
                        self.basis,
                        Position::new(origin[0], origin[1], origin[2]),
                        width,
                        pixels,
                    );
                    if g.draw(
                        &mut self.slot,
                        &GpuPlot::Slice(plot),
                        &slice_scheme(g.n_materials()),
                        eframe::egui_wgpu::wgpu::FilterMode::Nearest,
                        true,
                    ) {
                        let c = match self.basis {
                            PlotBasis::Xy => [origin[0], origin[1]],
                            PlotBasis::Xz => [origin[0], origin[2]],
                            PlotBasis::Yz => [origin[1], origin[2]],
                        };
                        self.gpu_place = Some((c, width));
                        self.dirty = false;
                        // A CPU slice still in flight belongs to an older view.
                        self.pending = None;
                    }
                }
            }
            if self.dirty || self.slot.busy() {
                ui.ctx().request_repaint();
            }
        }
        if !gpu_ready
            && self.dirty
            && have_geometry
            && self.pending.is_none()
            && now - self.changed_at > 0.3
        {
            let (origin, width, pixels) = self.window(rect, 1000.0);
            let id = self.next_id;
            self.next_id += 1;
            self.pending = Some(id);
            self.dirty = false;
            send(Req::Render {
                id,
                basis: self.basis,
                origin,
                width,
                pixels,
            });
        }
        if self.dirty || self.pending.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        rect
    }
}

impl SliceView {
    /// Whether a GPU slice has reached the screen since the last call: what
    /// the CPU path reports with an `Ev::Slice` (the review gate counts the
    /// preset on screen as seen).
    pub fn take_gpu_shown(&mut self) -> bool {
        self.slot.take_done()
    }
}

/// The plane controls for the settings panel: basis and depth.
pub fn plane_controls(ui: &mut egui::Ui, sv: &mut SliceView) {
    ui.horizontal(|ui| {
        ui.label("Plane");
        for b in [PlotBasis::Xy, PlotBasis::Xz, PlotBasis::Yz] {
            if ui
                .selectable_label(sv.basis == b, RichText::new(b.name().to_uppercase()))
                .clicked()
            {
                sv.basis = b;
                sv.at_preset = None;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Position along the normal [cm]");
        if ui
            .add(
                egui::DragValue::new(&mut sv.depth)
                    .speed(0.5)
                    .range(-500.0..=500.0),
            )
            .changed()
        {
            sv.at_preset = None;
        }
    });
}

/// The colours of `outram_mc_libs`' `render_material_slice` (the CPU slice
/// and its PNG export): the HTR-10 palette, OpenMC's default colours past
/// it, a light grey outside the model.
fn slice_scheme(n_materials: usize) -> ColourScheme {
    let pal = palette();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let mut s = ColourScheme::new(
        PlotColourBy::Material,
        n_materials.max(pal.len()),
        &mut seed,
    )
    .with_background(Rgb::new(235, 235, 235));
    for (i, (c, _)) in pal.iter().enumerate() {
        s = s.with_colour(i, *c);
    }
    s
}

/// Legend rows for the materials a GPU slice shows (its per-pixel ids).
fn legend_from_ids(ids: &[i32]) -> Vec<(Color32, &'static str)> {
    let pal = palette();
    let mut seen = vec![false; pal.len()];
    for &i in ids {
        if let Some(s) = usize::try_from(i).ok().and_then(|i| seen.get_mut(i)) {
            *s = true;
        }
    }
    pal.iter()
        .zip(seen)
        .filter(|((_, label), s)| *s && !label.is_empty())
        .map(|((c, label), _)| (Color32::from_rgb(c.r, c.g, c.b), *label))
        .collect()
}

/// Upload a raw slice and work out which palette entries it shows.
fn upload(ctx: &egui::Context, image: &ImageData) -> (TextureHandle, Vec<(Color32, &'static str)>) {
    let mut bytes = Vec::with_capacity(image.pixels.len() * 3);
    for p in &image.pixels {
        bytes.extend_from_slice(&[p.r, p.g, p.b]);
    }
    let ci = egui::ColorImage::from_rgb([image.width, image.height], &bytes);
    let tex = ctx.load_texture("slice", ci, TextureOptions::NEAREST);
    let pal = palette();
    let mut legend = Vec::new();
    for (c, label) in &pal {
        if label.is_empty() {
            continue;
        }
        if image.pixels.iter().any(|p| p == c) {
            legend.push((Color32::from_rgb(c.r, c.g, c.b), *label));
        }
    }
    (tex, legend)
}
