// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.
//
// NEW WORK (gh:#587): the wgpu side of the GPU CSG ray tracer. The shader
// (`csg_trace.wgsl`) is an f32 transcription of this crate's CPU plotter
// (`csg::plot::{raytrace, slice}`, an OpenMC port, MIT); see its header.

//! **Ray tracing an assembled CSG geometry on the GPU** — the same three
//! pictures as the CPU plotter, from the same inputs:
//!
//! | [`GpuPlot`] | CPU reference |
//! |---|---|
//! | `Solid` | [`SolidRayTracePlot::create_image_with_ids`] (Phong, light, [`ClipPlane`] section) |
//! | `Wireframe` | [`WireframeRayTracePlot::create_image`] (x-ray: attenuation + outlines) |
//! | `Slice` | [`SlicePlot::id_map`] (a material lookup per pixel) |
//!
//! The geometry goes up once as a [`FlatGeometry`] (one storage buffer); a
//! render is one compute dispatch per band of rows (so no single submission
//! runs long enough to trip a driver watchdog) into a padded colour buffer,
//! copied into an `Rgba8Unorm` texture that an egui-wgpu renderer can show
//! directly ([`GpuFrame::view`]), plus a per-pixel id buffer
//! ([`CsgGpuRenderer::read_back`]) for the parity test against the CPU.
//!
//! **The CPU plotter stays the reference and the fallback.** This path is
//! `f32` (the CPU is `f64`), so a pixel on a boundary may differ; the
//! mismatch fraction is measured by `tests/gpu_csg_parity.rs` and recorded
//! there. Not carried: tori (refused by [`super::flat::flatten`]),
//! cell-coloured plots, slice `level` / overlap checks / mesh lines, and
//! wireframe thickness other than 1.
//!
//! Device limits used: 4 storage buffers and one uniform per stage, within
//! `wgpu::Limits::downlevel_defaults()`.

use super::flat::FlatGeometry;
use crate::csg::plot::{
    ClipPlane, ColourScheme, ImageData, PlotBasis, PlotColourBy, Projection, Rgb, SlicePlot,
    SolidRayTracePlot, WireframeRayTracePlot,
};

/// The WGSL source of the tracer.
pub const SHADER: &str = include_str!("csg_trace.wgsl");

/// Pixel id: no geometry along the ray, or outside the model.
pub const ID_BACKGROUND: i32 = -1;
/// Pixel id: upstream's overlap colour (an opaque material entered across a
/// lattice-tile edge, or a slice overlap).
pub const ID_OVERLAP: i32 = -2;
/// Pixel id: a void cell (slices only).
pub const ID_VOID: i32 = -3;

/// Bytes of the `Params` uniform (see the shader).
const PARAMS_BYTES: usize = 192;
/// Workgroup edge, as `@workgroup_size(8, 8)` in the shader.
const WG: u32 = 8;

/// Which picture to draw, with the CPU plotter's own description of it.
#[derive(Debug, Clone)]
pub enum GpuPlot {
    /// OpenMC's solid ray trace (plus this crate's section plane).
    Solid(SolidRayTracePlot),
    /// OpenMC's wireframe ray trace ("x-ray").
    Wireframe(WireframeRayTracePlot),
    /// A material slice.
    Slice(SlicePlot),
}

impl GpuPlot {
    /// Pixels across and down.
    #[must_use]
    pub fn pixels(&self) -> [usize; 2] {
        match self {
            Self::Solid(p) => p.camera.pixels,
            Self::Wireframe(p) => p.camera.pixels,
            Self::Slice(p) => p.pixels,
        }
    }
}

/// Why a GPU render could not be made. Every case means "draw on the CPU".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuRenderError {
    /// No geometry uploaded yet.
    NoGeometry,
    /// The plot asks for something the GPU tracer does not carry.
    Unsupported(&'static str),
    /// wgpu reported an error (shader, pipeline, map, device lost).
    Gpu(String),
}

impl std::fmt::Display for GpuRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGeometry => write!(f, "no geometry on the GPU"),
            Self::Unsupported(w) => write!(f, "not supported on the GPU: {w}"),
            Self::Gpu(e) => write!(f, "GPU error: {e}"),
        }
    }
}

impl std::error::Error for GpuRenderError {}

/// One finished (or in-flight) GPU picture.
///
/// The texture is `Rgba8Unorm`, `TEXTURE_BINDING | COPY_SRC`, as
/// `egui_wgpu::Renderer::register_native_texture` requires. Its contents are
/// ready once the queue has run the submissions [`CsgGpuRenderer::render`]
/// made; sampling it earlier is ordered after them by wgpu.
#[derive(Debug)]
pub struct GpuFrame {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// The picture.
    pub texture: wgpu::Texture,
    /// A view of [`Self::texture`], for binding.
    pub view: wgpu::TextureView,
    colour: wgpu::Buffer,
    aux: wgpu::Buffer,
    stride: usize,
}

/// The GPU ray tracer: a device, the two pipelines and the uploaded geometry.
///
/// Owns clones of the `wgpu` handles (they are reference-counted), so it can
/// share eframe's device or use a headless one from [`probe_renderer`].
#[derive(Debug)]
pub struct CsgGpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipe: wgpu::ComputePipeline,
    wire_pipe: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    geometry: Option<(wgpu::Buffer, usize)>,
    /// The uploaded geometry's universe graph (its `words` left behind), for
    /// [`FlatGeometry::subtree_shown`].
    graph: Option<FlatGeometry>,
}

fn pack(c: Rgb) -> u32 {
    u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16) | 0xFF00_0000
}

fn bytes_u32(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_le_bytes()).collect()
}

impl CsgGpuRenderer {
    /// Build the pipelines on `device`.
    ///
    /// # Errors
    /// [`GpuRenderError::Gpu`] if the shader or a pipeline does not compile
    /// on this device (captured with an error scope, not a panic).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, GpuRenderError> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("csg-trace"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let render_pipe = pipe("render");
        let wire_pipe = pipe("wire");
        if let Some(e) = crate::gpu::block_on(scope.pop()) {
            return Err(GpuRenderError::Gpu(e.to_string()));
        }
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-params"),
            size: PARAMS_BYTES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            render_pipe,
            wire_pipe,
            params,
            geometry: None,
            graph: None,
        })
    }

    /// The device this renderer draws with.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Upload a flattened geometry (replacing any earlier one).
    ///
    /// # Errors
    /// [`GpuRenderError::Unsupported`] when the buffer is larger than this
    /// device lets one storage binding be (`max_storage_buffer_binding_size`,
    /// 128 MiB by default); nothing is uploaded and the CPU draws instead.
    pub fn set_geometry(&mut self, flat: &FlatGeometry) -> Result<(), GpuRenderError> {
        let limit = u64::from(self.device.limits().max_storage_buffer_binding_size);
        if flat.bytes() as u64 > limit {
            self.clear_geometry();
            return Err(GpuRenderError::Unsupported(
                "a geometry larger than the device's storage-buffer limit",
            ));
        }
        let bytes = flat.to_le_bytes();
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-geometry"),
            size: bytes.len().max(4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buf, 0, &bytes);
        self.geometry = Some((buf, flat.n_materials));
        self.graph = Some(FlatGeometry {
            words: Vec::new(),
            ..flat.clone()
        });
        Ok(())
    }

    /// Whether a geometry is uploaded.
    #[must_use]
    pub fn has_geometry(&self) -> bool {
        self.geometry.is_some()
    }

    /// Forget the geometry.
    pub fn clear_geometry(&mut self) {
        self.geometry = None;
        self.graph = None;
    }

    /// **Draw `plot` with `scheme`'s colours.** Submits the work and returns
    /// at once; [`Self::wait`] blocks until it has run, [`Self::read_back`]
    /// fetches the pixels.
    ///
    /// # Errors
    /// [`GpuRenderError`] if nothing is uploaded or the plot asks for
    /// something the GPU tracer does not carry.
    pub fn render(
        &self,
        plot: &GpuPlot,
        scheme: &ColourScheme,
    ) -> Result<GpuFrame, GpuRenderError> {
        let Some((geo, n_mat_geo)) = &self.geometry else {
            return Err(GpuRenderError::NoGeometry);
        };
        if scheme.colour_by != PlotColourBy::Material {
            return Err(GpuRenderError::Unsupported("colour by cell"));
        }
        if let GpuPlot::Slice(s) = plot {
            if s.level.is_some() || s.show_overlaps || s.meshlines.is_some() {
                return Err(GpuRenderError::Unsupported(
                    "slice level, overlaps or mesh lines",
                ));
            }
        }
        let [w, h] = plot.pixels();
        if w == 0 || h == 0 {
            return Err(GpuRenderError::Unsupported("an empty image"));
        }
        let n_mat = scheme.colours.len().max(*n_mat_geo).max(1);
        let (params, mut mats, max_band) = self.params_for(plot, scheme, n_mat);
        // After the material table: which universes / lattices hold anything
        // drawn, so the tracer crosses a fully hidden subtree as one cell.
        // A slice draws every material, so it never skips.
        if let Some(graph) = &self.graph {
            let flags = match plot {
                GpuPlot::Slice(_) => {
                    vec![1; graph.universe_contents.len() + graph.lattice_universes.len()]
                }
                _ => {
                    let shown: Vec<bool> = (0..n_mat).map(|i| mats[4 * i + 1] & 1 == 1).collect();
                    graph.subtree_shown(&shown)
                }
            };
            mats.extend(flags);
            while mats.len() % 4 != 0 {
                mats.push(0);
            }
        }

        let dev = &self.device;
        let stride = w.div_ceil(64) * 64;
        let colour = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-colour"),
            size: (stride * h * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let aux = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-aux"),
            size: (w * h * 8) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mats_buf = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-materials"),
            size: (mats.len() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&mats_buf, 0, &bytes_u32(&mats));
        let texture = dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("csg-image"),
            size: wgpu::Extent3d {
                width: w as u32,
                height: h as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind = |pipe: &wgpu::ComputePipeline, all: bool| {
            let mut entries = vec![wgpu::BindGroupEntry {
                binding: 0,
                resource: self.params.as_entire_binding(),
            }];
            if all {
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: geo.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: mats_buf.as_entire_binding(),
                });
            }
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: colour.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 4,
                resource: aux.as_entire_binding(),
            });
            dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("csg-bind"),
                layout: &pipe.get_bind_group_layout(0),
                entries: &entries,
            })
        };
        let render_bg = bind(&self.render_pipe, true);

        // One submission per band of rows, each with its own `row0`.
        let mut p = params;
        let mut row0 = 0usize;
        while row0 < h {
            let rows = max_band.min(h - row0);
            p[3] = row0 as u32;
            p[4] = stride as u32;
            self.queue.write_buffer(&self.params, 0, &bytes_u32(&p));
            let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("csg-band"),
            });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("csg-render"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.render_pipe);
                pass.set_bind_group(0, &render_bg, &[]);
                pass.dispatch_workgroups((w as u32).div_ceil(WG), (rows as u32).div_ceil(WG), 1);
            }
            self.queue.submit(Some(enc.finish()));
            row0 += rows;
        }
        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("csg-finish"),
        });
        if matches!(plot, GpuPlot::Wireframe(_)) {
            let wire_bg = bind(&self.wire_pipe, false);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("csg-wire"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.wire_pipe);
            pass.set_bind_group(0, &wire_bg, &[]);
            pass.dispatch_workgroups((w as u32).div_ceil(WG), (h as u32).div_ceil(WG), 1);
        }
        enc.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &colour,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some((stride * 4) as u32),
                    rows_per_image: Some(h as u32),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: w as u32,
                height: h as u32,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(enc.finish()));
        Ok(GpuFrame {
            width: w,
            height: h,
            texture,
            view,
            colour,
            aux,
            stride,
        })
    }

    /// The uniform words (with `row0` and `stride` filled per band), the
    /// material table, and the band height.
    fn params_for(
        &self,
        plot: &GpuPlot,
        scheme: &ColourScheme,
        n_mat: usize,
    ) -> (Vec<u32>, Vec<u32>, usize) {
        let mut p = vec![0u32; PARAMS_BYTES / 4];
        let fl = |x: f64| (x as f32).to_bits();
        let mut mats = vec![0u32; n_mat * 4];
        for i in 0..n_mat {
            mats[4 * i] = pack(scheme.colours.get(i).copied().unwrap_or(Rgb::new(0, 0, 0)));
        }
        let [w, h] = plot.pixels();
        p[1] = w as u32;
        p[2] = h as u32;
        p[5] = n_mat as u32;
        // colours: background, overlap, wire, void (WHITE, as the CPU slice)
        p[44] = pack(scheme.background);
        p[45] = pack(scheme.overlap_colour);
        p[46] = pack(Rgb::new(0, 0, 0));
        p[47] = pack(Rgb::new(255, 255, 255));
        let camera_words = |p: &mut Vec<u32>, cam: &crate::csg::plot::Camera| {
            let m = cam.camera_to_model();
            p[8] = fl(cam.position.x);
            p[9] = fl(cam.position.y);
            p[10] = fl(cam.position.z);
            match cam.projection {
                Projection::Perspective { horizontal_fov_deg } => {
                    let dx = 2.0 * 10.0 * (0.5 * horizontal_fov_deg.to_radians()).tan();
                    let dy = cam.pixels[1] as f64 / cam.pixels[0] as f64 * dx;
                    p[15] = fl(dx);
                    p[19] = fl(dy);
                }
                Projection::Orthographic { width } => {
                    p[6] |= 2;
                    p[11] = fl(width);
                }
            }
            for row in 0..3 {
                for col in 0..3 {
                    p[12 + 4 * row + col] = fl(m[3 * row + col]);
                }
            }
        };
        let band = match plot {
            GpuPlot::Solid(s) => {
                p[0] = 0;
                p[7] = 1_000_000;
                camera_words(&mut p, &s.camera);
                let light = s.light_position.unwrap_or(s.camera.position);
                p[24] = fl(light.x);
                p[25] = fl(light.y);
                p[26] = fl(light.z);
                p[27] = fl(s.diffuse_fraction);
                if let Some(ClipPlane { normal, offset }) = s.clip {
                    p[6] |= 1;
                    p[28] = fl(normal[0]);
                    p[29] = fl(normal[1]);
                    p[30] = fl(normal[2]);
                    p[31] = fl(offset);
                }
                for i in 0..n_mat {
                    if s.opaque.get(i).copied().unwrap_or(false) {
                        mats[4 * i + 1] |= 1;
                    }
                }
                64
            }
            GpuPlot::Wireframe(x) => {
                p[0] = 1;
                p[7] = 1_000_000;
                camera_words(&mut p, &x.camera);
                if !x.wireframe_ids.is_empty() {
                    p[6] |= 4;
                }
                p[46] = pack(x.wireframe_colour);
                for i in 0..n_mat {
                    let xs = x.xs.get(i).copied().unwrap_or(1e6);
                    let outlined = x.wireframe_ids.is_empty() || x.wireframe_ids.contains(&i);
                    if xs != 0.0 || outlined {
                        mats[4 * i + 1] |= 1;
                    }
                    if x.wireframe_ids.contains(&i) {
                        mats[4 * i + 1] |= 2;
                    }
                    mats[4 * i + 2] = fl(x.xs.get(i).copied().unwrap_or(1e6));
                }
                16
            }
            GpuPlot::Slice(s) => {
                p[0] = 2;
                let start = s.pixel_centre(0, 0);
                let (ax, ay) = s.basis.axes();
                let mut du = [0.0; 3];
                let mut dv = [0.0; 3];
                du[ax] = s.width[0] / s.pixels[0] as f64;
                dv[ay] = s.width[1] / s.pixels[1] as f64;
                debug_assert!(matches!(
                    s.basis,
                    PlotBasis::Xy | PlotBasis::Xz | PlotBasis::Yz
                ));
                p[32] = fl(start.x);
                p[33] = fl(start.y);
                p[34] = fl(start.z);
                for k in 0..3 {
                    p[36 + k] = fl(du[k]);
                    p[40 + k] = fl(dv[k]);
                }
                256
            }
        };
        (p, mats, band)
    }

    /// Block until every submitted render has run.
    ///
    /// # Errors
    /// [`GpuRenderError::Gpu`] if the device is lost.
    pub fn wait(&self) -> Result<(), GpuRenderError> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|e| GpuRenderError::Gpu(format!("{e:?}")))
    }

    /// Fetch a frame's pixels and per-pixel ids (blocking): ids are colour
    /// indices, or [`ID_BACKGROUND`], [`ID_OVERLAP`], [`ID_VOID`]. For a
    /// wireframe plot the ids are the outline hashes, not materials.
    ///
    /// # Errors
    /// [`GpuRenderError::Gpu`] if the copy or the map fails.
    pub fn read_back(&self, frame: &GpuFrame) -> Result<(ImageData, Vec<i32>), GpuRenderError> {
        let colour = self.read_buffer(&frame.colour)?;
        let aux = self.read_buffer(&frame.aux)?;
        let (w, h) = (frame.width, frame.height);
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let c = colour[y * frame.stride + x];
                pixels.push(Rgb::new(c as u8, (c >> 8) as u8, (c >> 16) as u8));
            }
        }
        let ids = (0..w * h).map(|i| aux[2 * i] as i32).collect();
        Ok((
            ImageData {
                width: w,
                height: h,
                pixels,
            },
            ids,
        ))
    }

    /// A solid frame's boundary crossings per pixel (blocking): where the
    /// tracer spends its time.
    ///
    /// # Errors
    /// [`GpuRenderError::Gpu`] if the copy or the map fails.
    pub fn read_steps(&self, frame: &GpuFrame) -> Result<Vec<u32>, GpuRenderError> {
        Ok(self.read_work(frame)?.into_iter().map(|(c, _)| c).collect())
    }

    /// A solid frame's work per pixel (blocking): `(boundary crossings,
    /// silent voxel steps)`, each saturating at 65 535. Voxel steps are the
    /// grid index's ([`super::index`]); zero for a universe without one.
    ///
    /// # Errors
    /// [`GpuRenderError::Gpu`] if the copy or the map fails.
    pub fn read_work(&self, frame: &GpuFrame) -> Result<Vec<(u32, u32)>, GpuRenderError> {
        let aux = self.read_buffer(&frame.aux)?;
        Ok((0..frame.width * frame.height)
            .map(|i| (aux[2 * i + 1] & 0xFFFF, aux[2 * i + 1] >> 16))
            .collect())
    }

    /// Start fetching a frame's per-pixel ids **without blocking**: poll the
    /// result with [`PendingIds::try_take`] on later frames of a UI.
    #[must_use]
    pub fn read_ids_async(&self, frame: &GpuFrame) -> PendingIds {
        let size = frame.aux.size();
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-ids"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&frame.aux, 0, &staging, 0, size);
        self.queue.submit(Some(enc.finish()));
        let done = std::sync::Arc::new(std::sync::Mutex::new(None));
        let d2 = done.clone();
        staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            if let Ok(mut d) = d2.lock() {
                *d = Some(r.is_ok());
            }
        });
        PendingIds {
            device: self.device.clone(),
            staging,
            done,
            n: frame.width * frame.height,
        }
    }

    fn read_buffer(&self, src: &wgpu::Buffer) -> Result<Vec<u32>, GpuRenderError> {
        let size = src.size();
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("csg-readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        self.queue.submit(Some(enc.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.wait()?;
        match rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(GpuRenderError::Gpu(format!("{e:?}"))),
            Err(_) => return Err(GpuRenderError::Gpu("map callback never ran".into())),
        }
        let out = {
            let view = staging
                .slice(..)
                .get_mapped_range()
                .map_err(|e| GpuRenderError::Gpu(format!("{e:?}")))?;
            view.chunks_exact(4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect()
        };
        staging.unmap();
        Ok(out)
    }
}

/// Per-pixel ids on their way back from the GPU ([`CsgGpuRenderer::read_ids_async`]).
#[derive(Debug)]
pub struct PendingIds {
    device: wgpu::Device,
    staging: wgpu::Buffer,
    done: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
    n: usize,
}

impl PendingIds {
    /// The ids if they have arrived (`Some(Err)` if the copy failed), or
    /// `None` while they are still in flight. Never blocks.
    pub fn try_take(&self) -> Option<Result<Vec<i32>, GpuRenderError>> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let state = *self.done.lock().ok()?;
        match state? {
            false => Some(Err(GpuRenderError::Gpu("id read-back failed".into()))),
            true => {
                let ids = {
                    let view = match self.staging.slice(..).get_mapped_range() {
                        Ok(v) => v,
                        Err(e) => return Some(Err(GpuRenderError::Gpu(format!("{e:?}")))),
                    };
                    view.chunks_exact(8)
                        .take(self.n)
                        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect()
                };
                self.staging.unmap();
                Some(Ok(ids))
            }
        }
    }
}

/// A renderer on a headless device ([`crate::gpu::probe`]), or `None` when
/// this machine has no usable adapter (CI, a VM): draw on the CPU then.
#[must_use]
pub fn probe_renderer() -> Option<CsgGpuRenderer> {
    let ctx = crate::gpu::probe()?;
    CsgGpuRenderer::new(&ctx.device, &ctx.queue).ok()
}
