/// prelude is here for easy imports
pub mod prelude;

/// Compute-backend selector (`ComputeType` / `ThreadCount`) — the runtime CPU vs
/// wgpu resource switcher for the Walk-on-Spheres ensembles, mirroring the
/// `outram-mc-libs` `ComputeType`. Compiles on all targets (the GPU *body* is
/// gated, not this enum), so `Gpu` is always selectable and falls back to CPU.
pub mod compute;

/// import the nuclide enum
pub use fission_yields_data::prelude::Nuclide;
/// import all nuclides into this crate
pub use fission_yields_data::prelude::Nuclide::*;

/// this contains the raw information
/// based on pwr neutron spectrum
pub mod decay_xml_info_serde;

/// this is the struct that converts the SerdeNuclideData to
/// NuclideReactionAndDecayData
pub mod nuclide_reaction_and_decay_data;

/// this is the part that deals with decay simulation in lagrangian
/// or monte carlo bit
/// this part deals only with the terminal user interface
pub mod lagrangian_decay_simulator;

/// this is the part that deals with transmutation and fission
/// simulation in lagrangian
pub mod lagrangian_transmutation_and_fission_simulator;

/// Optional wgpu GPU acceleration for large Walk-on-Spheres ensembles. Compiled
/// only off Android (the workspace GPU/Android rule) **and off wasm**; the CPU
/// path in `lagrangian_diffusion::first_passage::ensemble` is always available
/// and is the trusted reference. See the module docs for the CPU-fallback
/// contract.
///
/// The wasm exclusion is not a policy choice but a type-system one: wgpu's
/// WebGPU backend holds `Rc<Cell<u32>>` internally, so `GpuContext` is `!Send`
/// there and the `static OnceLock<Option<GpuContext>>` this module caches it in
/// cannot compile. Reaching WebGPU from wasm needs a `thread_local!` cache
/// instead — a real change, not a gate — and is tracked separately.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub mod gpu;

/// Eulerian / continuum-diffusion TRISO fission-product release — a Rust fork of
/// Idaho National Laboratory's TRISO-ATOPS (MIT). This is the continuum
/// complement to the crate's Lagrangian (single-atom Monte-Carlo) model: it uses
/// closed-form analytical solutions to the Fickian diffusion equation (Booth,
/// breakthrough, graphite-attenuation models) to predict per-nuclide release
/// fractions. See `docs/triso-atops-fork.md` and the module-level docs.
pub mod triso_atops_fork;

/// Serial stand-ins for the `rayon` surface this crate uses, on `wasm32` where
/// `rayon` does not build. See the module docs.
#[cfg(target_arch = "wasm32")]
mod wasm_par;
