/// Convenience re-export of the most commonly used types.
///
/// ```rust
/// use outram_mc_libs::prelude::*;
/// ```

pub use crate::rng::lcg::{prn, future_seed, init_seed};
pub use crate::geometry::position::{Position, Direction};
pub use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane, ZCylinder};
pub use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
pub use crate::geometry::universe::Universe;
pub use crate::geometry::lattice::{HexLattice, HexOrientation, Lattice, RectLattice};
pub use crate::geometry::virtual_lattice::{BuildReport, VirtualLattice};
pub use crate::geometry::geometry::{BoundaryHit, Coord, Crossing, Geometry, GeometryPath};
pub use crate::particle::particle::{Particle, ParticleType};
pub use crate::material::material::{MacroXs, Material};
pub use crate::material::nuclide::{MicroXS, Nuclide};
pub use crate::material::thermal::{
    CoherentElasticTable, IncoherentElasticTable, ThermalElastic, ThermalScattering,
};
pub use crate::tally::tally::{ScoreType, Tally, TallyBin};
pub use crate::tally::filter::{
    CellFilter, EnergyFilter, Filter, LegendreAxis, MaterialFilter, MeshFilter,
    SpatialLegendreFilter, UniverseFilter,
};
pub use crate::tally::mesh::RegularMesh;
pub use crate::tally::scoring::Q_FISSION_J;
pub use crate::tally::arithmetic::DerivedTally;
pub use crate::physics::compute::{ComputeType, ThreadCount};
pub use crate::physics::keff::{run_keff, KeffResult, KeffSettings};
pub use crate::physics::search::{
    search_for_keff, SearchError, SearchIteration, SearchMethod, SearchResult, SearchSettings,
};
pub use crate::physics::transport_csg::{run_keff_csg, SourceBox};
pub use crate::physics::fixed_source::{
    run_fixed_source, FixedSource, FixedSourceResult, FixedSourceSettings,
};
pub use crate::pebble_beds::delta_tracking::{track_to_collision, DeltaEvent, DeltaFlight, Majorant};
pub use crate::pebble_beds::keff_delta::run_keff_delta;
pub use crate::pebble_beds::sphere_packing::{pack_spheres, PackedSpheres, PackingConfig, PackingMethod};
// Stochastic-media research track (scaffold, beads epic op-eby). The chord
// statistics, SCLS retention machinery and brute-force index are implemented; the
// CLS/SCLS transport drivers are not — those paths return typed NotImplemented
// errors. See `stochastic::medium` for the model-dispatch enum.
pub use crate::stochastic::medium::{MaterialId, MediumError, RsaMedium, StochasticMedium};
pub use crate::stochastic::cls::{
    mean_chord_length_sphere, matrix_mean_chord_length, sample_chord, ClsMedium,
};
pub use crate::stochastic::scls::{
    AdaptiveRadius, FlightSegment, InclusionSphere, ParticleHistory, SclsMedium,
};
pub use crate::stochastic::spatial_index::{BruteForceIndex, IndexError, KdTreeIndex, SpatialIndex};
pub use crate::stochastic::benchmark::{AbsorptionBenchmark, BenchmarkResult};
// Optional GPU compute (headless wgpu). `GpuContext` + `gpu_probe` are available
// on every target (Android gets the CPU-only shim; `gpu_probe` there is always
// `None`). `interp_xs_cpu` is the trusted f64 reference; `interp_xs_gpu` is the
// f32 accelerated path, present only off Android.
pub use crate::gpu::xs_interp::interp_xs_cpu;
pub use crate::gpu::{probe as gpu_probe, GpuContext};
#[cfg(not(target_os = "android"))]
pub use crate::gpu::xs_interp::interp_xs_gpu;
// Ray–surface distance for all 15 CSG SurfaceKind variants (op-9s8.2). The
// encoding (`EncodedSurfaces`/`encode_surfaces`), the query type (`SurfaceQuery`),
// the `MISS`/`SURF_STRIDE` constants, and the scalar f32 CPU mirror
// (`surface_distance_cpu_f32`) build on every target; the GPU dispatch
// `surface_distance_gpu` is present only off Android.
pub use crate::gpu::surface_distance::{
    encode_surfaces, surface_distance_cpu_f32, EncodedSurfaces, SurfaceQuery, MISS, SURF_STRIDE,
};
#[cfg(not(target_os = "android"))]
pub use crate::gpu::surface_distance::surface_distance_gpu;
// Batched next-event GPU flight kernel (event-based transport). `FlightBatch`,
// `FlightSphere`, `FlightOutcome`, and `advance_flight_cpu_mirror` build on every
// target; the GPU dispatch `advance_flight_gpu` is present only off Android.
pub use crate::gpu::batched_flight::{
    advance_flight_cpu_mirror, FlightBatch, FlightOutcome, FlightSphere,
};
#[cfg(not(target_os = "android"))]
pub use crate::gpu::batched_flight::advance_flight_gpu;
// Fused flight + collision GPU event kernel (op-u6s.8 collision-on-GPU). The
// per-nuclide reaction tables (`CollisionTables`), the packed `EventTablesF32`,
// the SoA `EventBatch`/`EventSphere`, and the CPU-mirror drivers build on every
// target; the resident GPU driver `advance_generation_gpu` is present only off
// Android.
pub use crate::gpu::collision_grid::CollisionTables;
pub use crate::gpu::batched_event::{
    advance_event_cpu_mirror, advance_generation_cpu_mirror, EventBatch, EventSphere,
    EventTablesF32, FISS_NONE,
};
#[cfg(not(target_os = "android"))]
pub use crate::gpu::batched_event::advance_generation_gpu;
// Per-machine performance-report generator (detects host GPU/CPU/OS; renders a
// local, gitignored "what performance is available on my PC" markdown report).
pub use crate::perf_report::{HardwareInfo, PerfReport, PerfRow};
