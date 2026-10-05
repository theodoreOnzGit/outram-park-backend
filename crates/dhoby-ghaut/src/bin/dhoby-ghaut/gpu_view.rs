//! GPU drawing for the 3D viewport and the 2D slices (gh:#587).
//!
//! Maintainer, 2026-10-05: "ray-tracing should be the GPU's job in
//! dhoby-ghaut". The assembled geometry is flattened once
//! (`outram_blender::csg::gpu::flat`, on a thread of its own, not the UI
//! thread) and uploaded to eframe's own wgpu device; every picture is then
//! ray traced by `outram_blender::csg::gpu::render`, a compute shader that
//! walks the same cells, universes and lattices as the CPU plotter (the
//! drawing rule: draw what the solver sees), into a texture egui shows
//! directly. The UI thread only submits GPU work; it never waits for it.
//!
//! **The CPU plotter on the geometry engine thread stays the fallback**: no
//! wgpu renderer (eframe on glow), a shader the adapter refuses, a geometry
//! the GPU tracer does not carry (a torus), or the moment between assembly
//! and the flattened geometry reaching the GPU. The review gate's PNGs and
//! `--render-review` are always drawn on the CPU, the reference.

use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui_wgpu;
use nee_soon::htr10_rmc::core_model::AssembledCore;
use outram_blender::csg::gpu::flat::{flatten, FlatGeometry, FlattenError};
use outram_blender::csg::gpu::render::{CsgGpuRenderer, GpuFrame, GpuPlot, PendingIds};
use outram_mc_libs::geometry::plot::ColourScheme;

/// What the GPU side is doing, for the status line.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuStatus {
    /// No GPU renderer: everything is drawn on the CPU (the reason).
    Off(String),
    /// Flattening and uploading the assembled geometry.
    Preparing,
    /// Ready: pictures are traced on the GPU.
    Ready,
    /// This geometry cannot be traced on the GPU (the reason); CPU.
    Refused(String),
}

struct State {
    rs: egui_wgpu::RenderState,
    renderer: CsgGpuRenderer,
    incoming: Option<Receiver<Result<FlatGeometry, FlattenError>>>,
    status: GpuStatus,
    /// Material table length of the uploaded geometry.
    n_materials: usize,
    /// Bumped on every new geometry, so a view knows its picture is stale.
    generation: u64,
}

/// The shared GPU renderer (cheap to clone; the 3D view and the slice view
/// hold one each).
#[derive(Clone)]
pub struct Gpu(Arc<Mutex<State>>);

/// One view's picture on the GPU: its texture, registered with egui, and
/// the timing of the last render.
pub struct GpuSlot {
    tex: Option<egui::TextureId>,
    /// Keeps the texture alive while egui draws it.
    frame: Option<GpuFrame>,
    /// Set by the queue when the last render has run: its wall time [s].
    done: Arc<Mutex<Option<f64>>>,
    busy: bool,
    pub last_seconds: f64,
    generation: u64,
    ids: Option<PendingIds>,
    /// A render finished since [`Self::take_done`] last looked.
    just_done: bool,
}

impl GpuSlot {
    pub fn new() -> Self {
        Self {
            tex: None,
            frame: None,
            done: Arc::new(Mutex::new(None)),
            busy: false,
            last_seconds: 0.0,
            generation: u64::MAX,
            ids: None,
            just_done: false,
        }
    }

    /// The texture to draw, if a GPU picture of the current geometry exists.
    pub fn texture(&self, gpu: &Gpu) -> Option<egui::TextureId> {
        let g = gpu.0.lock().ok()?;
        (self.frame.is_some() && self.generation == g.generation).then_some(self.tex?)
    }

    /// Forget the picture.
    pub fn clear(&mut self) {
        self.frame = None;
        self.ids = None;
    }

    /// Whether the last render is still running on the GPU. Collects its
    /// time when it has finished.
    pub fn busy(&mut self) -> bool {
        if self.busy {
            if let Some(s) = self.done.lock().ok().and_then(|mut d| d.take()) {
                self.busy = false;
                self.last_seconds = s;
                self.just_done = true;
            }
        }
        self.busy
    }

    /// Whether a render has finished since the last call (once per render).
    pub fn take_done(&mut self) -> bool {
        self.busy();
        std::mem::take(&mut self.just_done)
    }

    /// The last render's per-pixel material ids, once they are back.
    pub fn take_ids(&mut self) -> Option<Vec<i32>> {
        let r = self.ids.as_ref()?.try_take()?;
        self.ids = None;
        r.ok()
    }
}

impl Gpu {
    /// A GPU renderer on eframe's own device, or `None` (with eframe on
    /// glow, or a device that refuses the shader): the CPU draws then.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let rs = cc
            .wgpu_render_state
            .clone()
            .ok_or("eframe is not running on wgpu")?;
        let renderer = CsgGpuRenderer::new(&rs.device, &rs.queue).map_err(|e| e.to_string())?;
        let info = rs.adapter.get_info();
        eprintln!(
            "dhoby-ghaut: GPU ray tracing on {} ({:?}, {:?})",
            info.name, info.backend, info.device_type
        );
        Ok(Self(Arc::new(Mutex::new(State {
            rs,
            renderer,
            incoming: None,
            status: GpuStatus::Preparing,
            n_materials: 0,
            generation: 0,
        }))))
    }

    /// A new assembled geometry: flatten it on a thread of its own; it goes
    /// to the GPU when [`Self::poll`] finds it done.
    pub fn set_core(&self, core: Arc<AssembledCore>, ctx: &egui::Context) {
        let Ok(mut s) = self.0.lock() else { return };
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(flatten(&core.geometry));
            ctx.request_repaint();
        });
        s.renderer.clear_geometry();
        s.incoming = Some(rx);
        s.status = GpuStatus::Preparing;
        s.generation += 1;
    }

    /// Take a finished flattening, upload it; report whether pictures can
    /// be traced on the GPU now.
    pub fn poll(&self) -> bool {
        let Ok(mut s) = self.0.lock() else {
            return false;
        };
        if let Some(rx) = &s.incoming {
            if let Ok(result) = rx.try_recv() {
                s.incoming = None;
                match result {
                    Ok(flat) => {
                        s.n_materials = flat.n_materials;
                        s.renderer.set_geometry(&flat);
                        s.status = GpuStatus::Ready;
                    }
                    Err(e) => s.status = GpuStatus::Refused(e.to_string()),
                }
            }
        }
        s.status == GpuStatus::Ready
    }

    pub fn status(&self) -> GpuStatus {
        self.0
            .lock()
            .map_or(GpuStatus::Off("lock poisoned".into()), |s| s.status.clone())
    }

    /// Materials in the uploaded geometry (`material_count`).
    pub fn n_materials(&self) -> usize {
        self.0.lock().map_or(0, |s| s.n_materials)
    }

    /// Trace `plot` into `slot`'s texture. Returns `false` (draw on the CPU)
    /// if the GPU cannot. `want_ids` also starts fetching the per-pixel
    /// material ids (for a legend).
    pub fn draw(
        &self,
        slot: &mut GpuSlot,
        plot: &GpuPlot,
        scheme: &ColourScheme,
        filter: egui_wgpu::wgpu::FilterMode,
        want_ids: bool,
    ) -> bool {
        let Ok(s) = self.0.lock() else { return false };
        if s.status != GpuStatus::Ready {
            return false;
        }
        let start = Instant::now();
        let frame = match s.renderer.render(plot, scheme) {
            Ok(f) => f,
            Err(_) => return false,
        };
        let done = slot.done.clone();
        if let Ok(mut d) = done.lock() {
            *d = None;
        }
        s.rs.queue.on_submitted_work_done(move || {
            if let Ok(mut d) = done.lock() {
                *d = Some(start.elapsed().as_secs_f64());
            }
        });
        slot.busy = true;
        {
            let mut r = s.rs.renderer.write();
            match slot.tex {
                Some(id) => {
                    r.update_egui_texture_from_wgpu_texture(&s.rs.device, &frame.view, filter, id)
                }
                None => {
                    slot.tex = Some(r.register_native_texture(&s.rs.device, &frame.view, filter))
                }
            }
        }
        slot.ids = want_ids.then(|| s.renderer.read_ids_async(&frame));
        slot.frame = Some(frame);
        slot.generation = s.generation;
        true
    }
}
