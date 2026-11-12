// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, version 3.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Affero General Public License for more details.
//! **The DEM timestep on the GPU** (gh:#1250, maintainer 2026-11-05: "start a
//! wgsl option for LIGGGHTS, within the LIGGGHTS crate, use this as a second
//! option for the pour web demo").
//!
//! [`GpuDem`] runs [`GranularSystem`]'s step — Hertz normal force with the
//! history tangential model, kick-drift-kick velocity-Verlet, the analytic
//! cylinder and one-sided walls and a static triangle-mesh wall — as WGSL
//! compute kernels (`shaders/dem.wgsl`), a translation of this crate's own
//! CPU path, which is LIGGGHTS-PUBLIC's. It covers what the HTR-10 pour uses
//! and refuses anything else ([`GpuDem::from_system`]).
//!
//! # Trust model: NOT bit-identical, and that is the point to know
//!
//! The CPU path is the trusted reference and stays the default. This backend
//! is the crate's first that is **not** bit-identical to it:
//!
//! - **`f32`.** WebGPU has no `f64`. Positions are carried as two `f32`s
//!   (`hi + lo`, updated by an exact two-sum), so a pair's separation keeps
//!   ~1e-12 m resolution: a plain `f32` at 2 m has a 2.4e-7 m step, about 3 %
//!   of the 9 µm overlap of a pebble near a bed's top. **The overlap** `δ =
//!   (r_i + r_j) − |x_i − x_j|` is then formed in double-f32 (radii as hi +
//!   lo too; Dekker's split for exact products), so it keeps ~1e-7 of itself: in plain
//!   `f32` the cancellation left ~4e-9 m, which the first comparison measured
//!   (a head-on rebound 1.8e-5 off the CPU's, 2026-11-05). Velocities, spins,
//!   forces and springs are `f32` (~1e-7 relative).
//! - **Summation order.** Each particle sums its contacts in cell order, not
//!   the CPU's pair order.
//!
//! A dense pour is chaotic, so these differences grow; agreement is checked
//! statistically (packing fraction, g(r), surface) and at early time per
//! pebble, never bit for bit. The GPU result is **deterministic**: each
//! cell's entries are sorted by index before the force pass, so the same
//! input gives the same output run to run.
//!
//! # Layout (one thread per particle; see the shader's header)
//!
//! - **Gather, not scatter.** Each particle computes the force of each of its
//!   contacts on itself. A pair's force is antisymmetric (`e_n`, `v_tr` and
//!   the spring flip sign from the other side), so the two sides compute
//!   exactly opposite halves, and each keeps its own copy of the spring.
//! - **Shear history** in fixed slots per particle (`K_SLOTS` pairs and three
//!   walls), keyed by the neighbour's index, double-buffered: a contact not
//!   touched in a step is dropped, as `ShearHistory::end_step` does.
//! - **Cell list** of edge `2 r_max`, at most `CELL_CAP` entries a cell;
//!   overflows of either table are counted and reported ([`GpuDem::errors`]).

use crate::boundary::Boundary;
use crate::granular::{GranularNormalModel, RollingModel, TangentialModel};
use crate::granular_system::GranularSystem;
use crate::mesh_wall::WallGeometry;
use crate::particle::Vec3;

use super::{bytes_of, slice_bytes, GpuContext};

/// Pair-contact slots per particle: a dense random packing has ~6–12
/// contacts; 16 leaves margin, and an overflow is counted, not hidden.
pub const K_SLOTS: u32 = 16;
/// Entries per cell (edge one pebble diameter: at most 8 non-overlapping
/// centres fit).
pub const CELL_CAP: u32 = 12;
const WG: u32 = 64;

/// The uniform block (must match `Params` in `dem.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Params {
    n: u32,
    k_slots: u32,
    cap: u32,
    n_tris: u32,
    dims: [u32; 4],
    origin: [f32; 4],
    gravity: [f32; 4],
    material: [f32; 4],
    cyl: [f32; 4],
    valve: [f32; 4],
    flags: [u32; 4],
    hull: [f32; 4],
    /// Ones the shader multiplies intermediates by so no compiler can
    /// reassociate them away (`keep` in `dem.wgsl`).
    one: [f32; 4],
}

/// A particle as the shader holds it (must match `P`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GpuParticle {
    hi: [f32; 4],
    lo: [f32; 4],
    v: [f32; 4],
    w: [f32; 4],
}

/// One particle read back: position (hi + lo summed in `f64`), velocity,
/// angular velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
}

/// What the tables could not hold (each count is of events, summed over
/// steps since the start): a cell with more than [`CELL_CAP`] entries, a
/// particle with more than [`K_SLOTS`] contacts. Either makes the result
/// wrong; a run with a nonzero count must not be trusted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuErrors {
    pub cell_overflow: u32,
    pub slot_overflow: u32,
}

/// The pure-data set-up of a GPU run: everything [`GpuDem`] uploads, built
/// from a [`GranularSystem`] and checked ([`GpuSetup::from_system`]). Pure
/// Rust, so it is tested without a GPU.
#[derive(Clone, Debug)]
pub struct GpuSetup {
    params: Params,
    particles: Vec<GpuParticle>,
    /// Six `vec4`s per triangle: a (hi), a (lo), b − a, c − a, normal,
    /// bounds (centroid, radius).
    tris: Vec<[f32; 4]>,
}

fn split(x: f64) -> (f32, f32) {
    let hi = x as f32;
    (hi, (x - f64::from(hi)) as f32)
}

fn f4(v: Vec3, w: f64) -> [f32; 4] {
    [v.x as f32, v.y as f32, v.z as f32, w as f32]
}

impl GpuSetup {
    /// The GPU set-up of `sys`'s current state, or why it is not supported.
    ///
    /// Supported: Hertz normal force, the history tangential model, no
    /// rolling resistance; analytic walls of at most one cylinder with a +z
    /// axis and one one-sided wall with normal ±z; at most one moving wall,
    /// a triangle mesh at rest. That is the HTR-10 pour
    /// (`htr10_fill::Htr10Fill`). Anything else is refused rather than run
    /// wrong.
    ///
    /// # Errors
    /// A description of the first unsupported feature.
    pub fn from_system(sys: &GranularSystem) -> Result<Self, String> {
        let model = sys.model();
        let (material, tangential_damping) = match model.normal {
            GranularNormalModel::Hertz {
                material,
                tangential_damping,
            } => (material, tangential_damping),
            _ => return Err("the GPU path has the Hertz normal model only".into()),
        };
        if !matches!(model.tangential, TangentialModel::History) {
            return Err("the GPU path has the history tangential model only".into());
        }
        if !matches!(model.rolling, RollingModel::Off) {
            return Err("the GPU path has no rolling resistance (µ_r must be 0)".into());
        }
        let mut cyl = None;
        let mut valve = None;
        for b in sys.boundaries() {
            match *b {
                Boundary::Cylinder {
                    axis_point,
                    axis_dir,
                    radius,
                } if cyl.is_none() => {
                    let a = axis_dir.scale(1.0 / axis_dir.norm());
                    if (a.z.abs() - 1.0).abs() > 1e-12 {
                        return Err("the GPU path's cylinder must have a z axis".into());
                    }
                    let (rh, rl) = split(radius);
                    cyl = Some([axis_point.x as f32, axis_point.y as f32, rh, rl]);
                }
                Boundary::Wall { point, normal } if valve.is_none() => {
                    let n = normal.scale(1.0 / normal.norm());
                    if (n.z.abs() - 1.0).abs() > 1e-12 {
                        return Err("the GPU path's one-sided wall must have a ±z normal".into());
                    }
                    let (zh, zl) = split(point.z);
                    valve = Some([zh, n.z.signum() as f32, zl, 0.0]);
                }
                _ => return Err(format!("the GPU path cannot hold this boundary: {b:?}")),
            }
        }
        let mut tris: Vec<[f32; 4]> = Vec::new();
        let mut hull = [0.0f32; 4];
        match sys.moving_walls() {
            [] => {}
            [w] => {
                if w.velocity.norm() != 0.0 || w.angular_velocity.norm() != 0.0 {
                    return Err("the GPU path's mesh wall must be at rest".into());
                }
                let WallGeometry::Mesh(mesh) = &w.geometry else {
                    return Err("the GPU path's moving wall must be a mesh".into());
                };
                // Bounding spheres: centroid and farthest vertex, the same
                // exact bound `MeshWall` prunes with (any true bound gives
                // the same answer).
                let mut centroids = Vec::new();
                for t in &mesh.triangles {
                    let c = t.a.add(t.b).add(t.c).scale(1.0 / 3.0);
                    let r = [t.a, t.b, t.c]
                        .iter()
                        .map(|v| v.sub(c).norm())
                        .fold(0.0, f64::max);
                    let (ax, ay, az) = (split(t.a.x), split(t.a.y), split(t.a.z));
                    tris.extend([
                        [ax.0, ay.0, az.0, 0.0],
                        [ax.1, ay.1, az.1, 0.0],
                        f4(t.b.sub(t.a), 0.0),
                        f4(t.c.sub(t.a), 0.0),
                        f4(t.normal(), 0.0),
                        f4(c, r * (1.0 + 1e-6) + 1e-9),
                    ]);
                    centroids.push((c, r));
                }
                let n = centroids.len().max(1) as f64;
                let hc = centroids
                    .iter()
                    .fold(Vec3::zero(), |s, (c, _)| s.add(*c))
                    .scale(1.0 / n);
                let hr = centroids
                    .iter()
                    .map(|(c, r)| c.sub(hc).norm() + r)
                    .fold(0.0, f64::max);
                hull = f4(hc, hr * (1.0 + 1e-6) + 1e-9);
            }
            _ => return Err("the GPU path holds at most one mesh wall".into()),
        }

        let ps = sys.particles();
        if ps.is_empty() {
            return Err("no particles".into());
        }
        if ps.len() >= 1 << 24 {
            return Err(
                "the GPU path holds fewer than 2^24 particles (an index is held as an f32 value)"
                    .into(),
            );
        }
        let r_max = ps.iter().map(|p| p.radius).fold(0.0, f64::max);
        let edge = 2.0 * r_max;
        let (mut lo, mut hi) = (
            Vec3::new(f64::MAX, f64::MAX, f64::MAX),
            Vec3::new(f64::MIN, f64::MIN, f64::MIN),
        );
        for p in ps {
            lo = Vec3::new(
                lo.x.min(p.position.x),
                lo.y.min(p.position.y),
                lo.z.min(p.position.z),
            );
            hi = Vec3::new(
                hi.x.max(p.position.x),
                hi.y.max(p.position.y),
                hi.z.max(p.position.z),
            );
        }
        // The grid covers the vessel (the cylinder's radius when there is
        // one) and the particles, plus a cell each side; particles that leave
        // it are clamped into the edge cells, which only costs speed.
        let reach = cyl.map_or(0.0, |c| f64::from(c[2]) + f64::from(c[3]));
        lo = Vec3::new(
            lo.x.min(-reach) - edge,
            lo.y.min(-reach) - edge,
            lo.z - edge,
        );
        hi = Vec3::new(hi.x.max(reach) + edge, hi.y.max(reach) + edge, hi.z + edge);
        let dims = [
            ((hi.x - lo.x) / edge).ceil().max(1.0) as u32,
            ((hi.y - lo.y) / edge).ceil().max(1.0) as u32,
            ((hi.z - lo.z) / edge).ceil().max(1.0) as u32,
        ];
        let total = dims[0] * dims[1] * dims[2];
        let g = sys.gravity();
        let params = Params {
            n: ps.len() as u32,
            k_slots: K_SLOTS,
            cap: CELL_CAP,
            n_tris: (tris.len() / 6) as u32,
            dims: [dims[0], dims[1], dims[2], total],
            origin: [lo.x as f32, lo.y as f32, lo.z as f32, edge as f32],
            gravity: [g.x as f32, g.y as f32, g.z as f32, sys.dt() as f32],
            material: [
                material.y_eff() as f32,
                material.g_eff() as f32,
                material.beta_eff() as f32,
                material.friction as f32,
            ],
            cyl: cyl.unwrap_or([0.0; 4]),
            valve: valve.unwrap_or([0.0; 4]),
            flags: [
                u32::from(cyl.is_some()),
                u32::from(valve.is_some()),
                u32::from(!tris.is_empty()),
                u32::from(tangential_damping),
            ],
            hull,
            one: [1.0; 4],
        };
        let particles = ps
            .iter()
            .map(|p| {
                let (x, y, z) = (
                    split(p.position.x),
                    split(p.position.y),
                    split(p.position.z),
                );
                GpuParticle {
                    hi: [x.0, y.0, z.0, p.radius as f32],
                    lo: [x.1, y.1, z.1, p.mass as f32],
                    // The radius as hi + lo too: the overlap is a difference
                    // of radii and a distance (shader, "double-f32").
                    v: f4(p.velocity, p.radius - f64::from(p.radius as f32)),
                    w: f4(p.angular_velocity, 0.0),
                }
            })
            .collect();
        Ok(Self {
            params,
            particles,
            tris,
        })
    }

    /// Particles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.particles.len()
    }

    /// Whether there are none (never, after [`Self::from_system`]).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// Grid cells (x, y, z) and the cell edge \[m\].
    #[must_use]
    pub fn grid(&self) -> ([u32; 3], f32) {
        let d = self.params.dims;
        ([d[0], d[1], d[2]], self.params.origin[3])
    }

    /// Mesh triangles uploaded.
    #[must_use]
    pub fn triangles(&self) -> u32 {
        self.params.n_tris
    }

    /// The particle states as the GPU would start from, read back to `f64`
    /// (the hi + lo split is exact to ~1e-15 m).
    #[must_use]
    pub fn states(&self) -> Vec<GpuState> {
        self.particles.iter().map(state_of).collect()
    }
}

fn state_of(p: &GpuParticle) -> GpuState {
    let s = |k: usize| f64::from(p.hi[k]) + f64::from(p.lo[k]);
    GpuState {
        position: Vec3::new(s(0), s(1), s(2)),
        velocity: Vec3::new(f64::from(p.v[0]), f64::from(p.v[1]), f64::from(p.v[2])),
        angular_velocity: Vec3::new(f64::from(p.w[0]), f64::from(p.w[1]), f64::from(p.w[2])),
    }
}

/// **A [`GranularSystem`] stepping on the GPU** (module docs).
pub struct GpuDem {
    ctx: GpuContext,
    setup: GpuSetup,
    pipelines: [wgpu::ComputePipeline; 6],
    groups: [wgpu::BindGroup; 2],
    state: wgpu::Buffer,
    ft: wgpu::Buffer,
    errs: wgpu::Buffer,
    steps: u64,
}

/// The kernels, in step order.
const KERNELS: [&str; 6] = [
    "kick_drift",
    "clear_cells",
    "bin",
    "sort_cells",
    "forces",
    "kick",
];

impl GpuDem {
    /// Upload `sys`'s state and build the kernels on `ctx`; the forces of the
    /// starting state are computed, as `GranularSystem::new` does.
    ///
    /// # Errors
    /// The system uses something the GPU path does not
    /// ([`GpuSetup::from_system`]).
    pub fn new(ctx: GpuContext, sys: &GranularSystem) -> Result<Self, String> {
        let setup = GpuSetup::from_system(sys)?;
        let device = &ctx.device;
        use wgpu::util::DeviceExt;
        let n = setup.particles.len() as u64;
        let p = setup.params;
        let storage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dem params"),
            contents: bytes_of(&p),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let state = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dem particles"),
            contents: slice_bytes(&setup.particles),
            usage: storage,
        });
        let zeros = |label: &str, bytes: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.max(16),
                usage: storage,
                mapped_at_creation: false,
            })
        };
        let ft = zeros("dem force/torque", n * 2 * 16);
        let cells = zeros(
            "dem cells",
            u64::from(p.dims[3]) * u64::from(1 + CELL_CAP) * 4,
        );
        // Every slot starts empty: neighbour −1, wall not live.
        let slots = (u64::from(K_SLOTS) + 3) * n;
        let mut empty: Vec<[f32; 4]> = Vec::with_capacity(slots as usize);
        for _ in 0..n {
            empty.extend((0..K_SLOTS).map(|_| [0.0, 0.0, 0.0, -1.0]));
            empty.extend((0..3).map(|_| [0.0f32; 4]));
        }
        let hist = [0, 1].map(|k| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(if k == 0 { "dem shear A" } else { "dem shear B" }),
                contents: slice_bytes(&empty),
                usage: storage,
            })
        });
        let tri_data: Vec<[f32; 4]> = if setup.tris.is_empty() {
            vec![[0.0; 4]]
        } else {
            setup.tris.clone()
        };
        let tris = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dem triangles"),
            contents: slice_bytes(&tri_data),
            usage: storage,
        });
        let errs = zeros("dem errors", 16);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dem"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/dem.wgsl").into()),
        });
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dem layout"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, rw),
                entry(2, rw),
                entry(3, rw),
                entry(4, rw),
                entry(5, rw),
                entry(6, rw),
                entry(7, rw),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dem pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = KERNELS.map(|k| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(k),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(k),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        });
        let group = |old: &wgpu::Buffer, new: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("dem group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: ft.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: cells.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: old.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: new.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: tris.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: errs.as_entire_binding(),
                    },
                ],
            })
        };
        let groups = [group(&hist[0], &hist[1]), group(&hist[1], &hist[0])];
        let mut dem = Self {
            ctx,
            setup,
            pipelines,
            groups,
            state,
            ft,
            errs,
            steps: 0,
        };
        // The starting forces (`GranularSystem::new` calls `compute_forces`).
        dem.encode(&[1, 2, 3, 4], 1);
        Ok(dem)
    }

    /// Record `steps` steps of the given kernels (indices into [`KERNELS`])
    /// and submit them.
    fn encode(&mut self, kernels: &[usize], steps: u64) {
        let p = self.setup.params;
        let particle_groups = p.n.div_ceil(WG);
        let cell_groups = p.dims[3].div_ceil(WG);
        let mut enc = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dem steps"),
            });
        for _ in 0..steps {
            // The force pass reads the springs of the last step and writes
            // this step's: the two tables swap every force pass.
            let g = &self.groups[(self.steps % 2) as usize];
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("dem step"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, g, &[]);
            for &k in kernels {
                pass.set_pipeline(&self.pipelines[k]);
                let groups = if k == 1 || k == 3 {
                    cell_groups
                } else {
                    particle_groups
                };
                pass.dispatch_workgroups(groups, 1, 1);
            }
            drop(pass);
            if kernels.contains(&4) {
                self.steps += 1;
            }
        }
        self.ctx.queue.submit(Some(enc.finish()));
    }

    /// Advance `n` steps (submitted in batches; returns once submitted, and
    /// [`Self::states`] waits for them).
    pub fn run(&mut self, n: u64) {
        let mut left = n;
        while left > 0 {
            let k = left.min(200);
            self.encode(&[0, 1, 2, 3, 4, 5], k);
            left -= k;
        }
    }

    /// Steps taken (counting the starting force pass as none).
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.steps.saturating_sub(1)
    }

    /// The set-up this run was built from.
    #[must_use]
    pub fn setup(&self) -> &GpuSetup {
        &self.setup
    }

    /// The adapter in use.
    #[must_use]
    pub fn adapter(&self) -> &wgpu::AdapterInfo {
        &self.ctx.info
    }

    /// Copy `bytes` of `buf` into a mappable buffer and submit; the caller
    /// maps it.
    fn readback(&self, buf: &wgpu::Buffer, bytes: u64) -> wgpu::Buffer {
        let device = &self.ctx.device;
        let rb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dem readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dem read"),
        });
        enc.copy_buffer_to_buffer(buf, 0, &rb, 0, bytes);
        self.ctx.queue.submit(Some(enc.finish()));
        rb
    }

    /// [`Self::read`] without blocking: the browser's way (it maps the
    /// buffer between tasks). Natively the device is polled until it does.
    async fn read_async(&self, buf: &wgpu::Buffer, bytes: u64) -> Option<Vec<u8>> {
        let rb = self.readback(buf, bytes);
        let shared = std::sync::Arc::new(std::sync::Mutex::new(super::Mapped::default()));
        rb.slice(..)
            .map_async(wgpu::MapMode::Read, super::mapped_callback(shared.clone()));
        #[cfg(not(target_arch = "wasm32"))]
        self.ctx
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .ok()?;
        if !super::MappedFuture(shared).await {
            return None;
        }
        let data = rb.slice(..).get_mapped_range().ok()?.to_vec();
        rb.unmap();
        Some(data)
    }

    /// Every particle's state now, without blocking (the browser's
    /// [`Self::states`]).
    pub async fn states_async(&self) -> Vec<GpuState> {
        let n = self.setup.particles.len();
        match self
            .read_async(&self.state, (n * std::mem::size_of::<GpuParticle>()) as u64)
            .await
        {
            Some(b) => decode_states(&b),
            None => Vec::new(),
        }
    }

    /// Overflow counts since the start, without blocking.
    pub async fn errors_async(&self) -> GpuErrors {
        decode_errors(self.read_async(&self.errs, 16).await)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read(&self, buf: &wgpu::Buffer, bytes: u64) -> Option<Vec<u8>> {
        let device = &self.ctx.device;
        let rb = self.readback(buf, bytes);
        let slice = rb.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let data = slice.get_mapped_range().ok()?.to_vec();
        rb.unmap();
        Some(data)
    }

    /// Every particle's state now (waits for the submitted steps).
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn states(&self) -> Vec<GpuState> {
        let n = self.setup.particles.len();
        self.read(&self.state, (n * std::mem::size_of::<GpuParticle>()) as u64)
            .map_or_else(Vec::new, |b| decode_states(&b))
    }

    /// Each particle's last computed (force, torque) (waits for the
    /// submitted steps): what a check compares with the CPU's contact law.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn forces(&self) -> Vec<(Vec3, Vec3)> {
        let n = self.setup.particles.len();
        let Some(b) = self.read(&self.ft, (n * 32) as u64) else {
            return Vec::new();
        };
        let f: Vec<f32> = b
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        f.chunks_exact(8)
            .map(|q| {
                (
                    Vec3::new(f64::from(q[0]), f64::from(q[1]), f64::from(q[2])),
                    Vec3::new(f64::from(q[4]), f64::from(q[5]), f64::from(q[6])),
                )
            })
            .collect()
    }

    /// Overflow counts since the start (waits for the submitted steps).
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn errors(&self) -> GpuErrors {
        decode_errors(self.read(&self.errs, 16))
    }
}

fn decode_states(bytes: &[u8]) -> Vec<GpuState> {
    bytes
        .chunks_exact(std::mem::size_of::<GpuParticle>())
        .map(|b| {
            let f =
                |k: usize| f32::from_ne_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]);
            let p = GpuParticle {
                hi: [f(0), f(1), f(2), f(3)],
                lo: [f(4), f(5), f(6), f(7)],
                v: [f(8), f(9), f(10), f(11)],
                w: [f(12), f(13), f(14), f(15)],
            };
            state_of(&p)
        })
        .collect()
}

/// A failed read reads as overflows (never as a clean run).
fn decode_errors(b: Option<Vec<u8>>) -> GpuErrors {
    match b {
        Some(b) if b.len() >= 8 => GpuErrors {
            cell_overflow: u32::from_ne_bytes([b[0], b[1], b[2], b[3]]),
            slot_overflow: u32::from_ne_bytes([b[4], b[5], b[6], b[7]]),
        },
        _ => GpuErrors {
            cell_overflow: u32::MAX,
            slot_overflow: u32::MAX,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::granular::{GranularContactModel, GranularMaterial};
    use crate::particle::Particle;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    fn ball(pos: Vec3, vel: Vec3) -> Particle {
        Particle::new(
            pos,
            vel,
            Vec3::zero(),
            Mass::new::<kilogram>(0.2),
            Length::new::<meter>(0.03),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("particle")
    }

    fn system(ps: Vec<Particle>, boundaries: Vec<Boundary>) -> GranularSystem {
        let m = GranularMaterial::new(5e8, 0.2, 0.5, 0.1).expect("material");
        GranularSystem::new(
            ps,
            boundaries,
            GranularContactModel::hertz_history(m),
            Vec3::new(0.0, 0.0, -9.81),
            3.5e-5,
        )
        .expect("system")
    }

    /// The shader avoids what Chrome's compiler (Tint) refuses and naga
    /// accepts, found 2026-11-05: a NaN bit pattern in a constant
    /// (`bitcast<f32>(0xffffffff)` made every pipeline invalid in the
    /// browser). Indices are f32 values, never bit patterns.
    #[test]
    fn the_shader_has_no_bit_pattern_floats() {
        let src = include_str!("shaders/dem.wgsl");
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("bitcast<f32>"),
            "a bit-pattern float in dem.wgsl"
        );
    }

    /// The set-up holds the state to the hi + lo split's precision, sizes
    /// the grid from the particles, and refuses what the shader lacks.
    #[test]
    fn the_setup_is_exact_and_refuses_what_the_gpu_lacks() {
        let p = ball(
            Vec3::new(0.123_456_789_012_345, -0.3, 2.000_000_123_456_7),
            Vec3::new(0.1, 0.0, -1.0),
        );
        let sys = system(
            vec![p, ball(Vec3::new(0.3, 0.3, 0.5), Vec3::zero())],
            vec![
                Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.9).expect("cyl"),
                Boundary::wall(Vec3::new(0.0, 0.0, -0.6), Vec3::new(0.0, 0.0, 1.0)).expect("wall"),
            ],
        );
        let s = GpuSetup::from_system(&sys).expect("supported");
        let back = s.states();
        assert!((back[0].position.x - 0.123_456_789_012_345).abs() < 1e-14);
        assert!((back[0].position.z - 2.000_000_123_456_7).abs() < 1e-14);
        let (dims, edge) = s.grid();
        assert_eq!(edge, 0.06);
        assert!(dims[0] >= 30 && dims[2] >= 2, "{dims:?}");
        assert_eq!(s.triangles(), 0);

        let m = GranularMaterial::new(5e8, 0.2, 0.5, 0.1).expect("material");
        let rolling = GranularSystem::new(
            vec![ball(Vec3::zero(), Vec3::zero())],
            vec![],
            GranularContactModel::hertz_history(m)
                .with_rolling(RollingModel::cdt(0.1).expect("cdt")),
            Vec3::zero(),
            1e-5,
        )
        .expect("system");
        assert!(GpuSetup::from_system(&rolling)
            .unwrap_err()
            .contains("rolling"));
        let plane = system(
            vec![ball(Vec3::zero(), Vec3::zero())],
            vec![Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("plane")],
        );
        assert!(GpuSetup::from_system(&plane).is_err());
    }
}
