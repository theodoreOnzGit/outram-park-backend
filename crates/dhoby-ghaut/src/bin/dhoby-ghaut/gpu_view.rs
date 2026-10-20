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
//!
//! The flattened geometry carries a grid index on its root universe
//! (`outram_blender::csg::gpu::index`, 2026-10-05): HTR-10's half-sections
//! trace in 20-60 ms instead of ~0.36 s, so the 3D view traces at full
//! resolution while the camera moves once a full frame takes under 30 ms.
//!
//! [`PebbleGpu`] draws Step 1's DEM pour (`crate::dem`) on the same device:
//! every pebble a sphere impostor (`pebbles.wgsl`), with a depth buffer, in
//! an offscreen target egui shows as a texture.

use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::future::Future;
use std::time::Instant;

use eframe::egui_wgpu;
use eframe::egui_wgpu::wgpu;
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
                        s.status = match s.renderer.set_geometry(&flat) {
                            Ok(()) => GpuStatus::Ready,
                            Err(e) => GpuStatus::Refused(e.to_string()),
                        };
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
        filter: wgpu::FilterMode,
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

// ── Step 1's DEM pour as sphere impostors ────────────────────────────────────

/// The camera of the pebble view, as `pebbles.wgsl`'s uniform reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PebbleCamera {
    pub yaw: f32,
    pub pitch: f32,
    /// NDC units per metre across and down.
    pub ndc_per_m: [f32; 2],
    /// z of the view centre \[m\].
    pub z_mid: f32,
    /// Pebble radius \[m\].
    pub radius: f32,
    /// Largest |depth| a pebble centre can have \[m\] (sets the depth range).
    pub depth_extent: f32,
    /// Show only the half y >= 0.
    pub half: bool,
}

struct PebbleTarget {
    size: [u32; 2],
    _colour: wgpu::Texture,
    view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    tex: egui::TextureId,
}

/// **The DEM pour drawn on the GPU** (gh:#587): every pebble a sphere
/// impostor (an instanced quad, an exact per-fragment ray-sphere hit with
/// its own depth) in an offscreen colour + depth target that egui shows as a
/// texture. Replaces painting 27 000 egui circles sorted on the CPU each
/// frame; the egui painter stays the fallback when eframe has no wgpu
/// device. The UI thread only uploads the centres and submits the pass.
pub struct PebbleGpu {
    rs: egui_wgpu::RenderState,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    instances: Option<(wgpu::Buffer, usize)>,
    target: Option<PebbleTarget>,
    /// Wall time of the last submitted frame's CPU side (upload + encode).
    pub last_submit_ms: f64,
}

const PEBBLE_SHADER: &str = include_str!("pebbles.wgsl");
const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

impl Gpu {
    /// eframe's wgpu state, for the other GPU views.
    pub fn render_state(&self) -> Option<egui_wgpu::RenderState> {
        self.0.lock().ok().map(|s| s.rs.clone())
    }
}

impl PebbleGpu {
    /// The pipeline on eframe's device, or the reason it cannot be built.
    pub fn new(rs: &egui_wgpu::RenderState) -> Result<Self, String> {
        let dev = &rs.device;
        let scope = dev.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dem-pebbles"),
            source: wgpu::ShaderSource::Wgsl(PEBBLE_SHADER.into()),
        });
        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dem-pebbles"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOUR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dem-pebbles-uniform"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dem-pebbles"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        if let Some(e) = pollster_free_pop(scope) {
            return Err(e);
        }
        Ok(Self {
            rs: rs.clone(),
            pipeline,
            uniform,
            bind,
            instances: None,
            target: None,
            last_submit_ms: 0.0,
        })
    }

    fn ensure_target(&mut self, size: [u32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let dev = &self.rs.device;
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let make = |format, usage| {
            dev.create_texture(&wgpu::TextureDescriptor {
                label: Some("dem-pebbles-target"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let colour = make(
            COLOUR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = make(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let view = colour.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let tex = {
            let mut r = self.rs.renderer.write();
            match self.target.take() {
                Some(old) => {
                    r.update_egui_texture_from_wgpu_texture(
                        dev,
                        &view,
                        wgpu::FilterMode::Linear,
                        old.tex,
                    );
                    old.tex
                }
                None => r.register_native_texture(dev, &view, wgpu::FilterMode::Linear),
            }
        };
        self.target = Some(PebbleTarget {
            size,
            _colour: colour,
            view,
            _depth: depth,
            depth_view,
            tex,
        });
    }

    /// Draw `centres` (metres, DEM frame) into a `size`-pixel picture and
    /// return its texture. Submits and returns at once.
    pub fn draw(
        &mut self,
        size: [u32; 2],
        centres: &[[f32; 3]],
        cam: &PebbleCamera,
    ) -> egui::TextureId {
        let t = std::time::Instant::now();
        let size = [size[0].clamp(1, 8192), size[1].clamp(1, 8192)];
        self.ensure_target(size);
        let dev = self.rs.device.clone();
        let queue = self.rs.queue.clone();
        let n = centres.len();
        if self
            .instances
            .as_ref()
            .is_none_or(|(_, cap)| *cap < n.max(1))
        {
            let cap = n.max(1).next_power_of_two();
            self.instances = Some((
                dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("dem-pebbles-centres"),
                    size: (cap * 12) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                cap,
            ));
        }
        let Some((inst, _)) = &self.instances else {
            unreachable!("created above")
        };
        if n > 0 {
            let bytes: Vec<u8> = centres
                .iter()
                .flat_map(|c| c.iter().flat_map(|v| v.to_le_bytes()))
                .collect();
            queue.write_buffer(inst, 0, &bytes);
        }
        let u: [f32; 12] = [
            cam.yaw.cos(),
            cam.yaw.sin(),
            cam.pitch.cos(),
            cam.pitch.sin(),
            cam.ndc_per_m[0],
            cam.ndc_per_m[1],
            cam.z_mid,
            cam.radius,
            0.5 / cam.depth_extent.max(1e-3),
            if cam.half { 1.0 } else { 0.0 },
            0.0,
            0.0,
        ];
        let ub: Vec<u8> = u.iter().flat_map(|v| v.to_le_bytes()).collect();
        queue.write_buffer(&self.uniform, 0, &ub);
        let Some(target) = &self.target else {
            unreachable!("ensured above")
        };
        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dem-pebbles"),
        });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dem-pebbles"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if n > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind, &[]);
                pass.set_vertex_buffer(0, inst.slice(..(n * 12) as u64));
                pass.draw(0..6, 0..n as u32);
            }
        }
        queue.submit(Some(enc.finish()));
        self.last_submit_ms = 1e3 * t.elapsed().as_secs_f64();
        target.tex
    }
}

/// Pop a validation error scope without blocking the UI thread: one poll
/// with a no-op waker. On native backends wgpu validates the creation calls
/// synchronously, so the result is ready; if it is not, assume success (a
/// later error surfaces through wgpu's uncaptured-error handler).
fn pollster_free_pop(scope: wgpu::ErrorScopeGuard) -> Option<String> {
    let mut fut = std::pin::pin!(scope.pop());
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match fut.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(e) => e.map(|e| e.to_string()),
        std::task::Poll::Pending => None,
    }
}
