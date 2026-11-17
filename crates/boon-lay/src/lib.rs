//! # BOON LAY
//!
//! **BO**mbardment of neutrons **O**n **N**uclides with **L**agrangian
//! transport **a**nd transmutation **Y**ields.
//!
//! TRISO-particle and Lagrangian decay / transmutation simulator, and host of
//! the TRISO-ATOPS fork (`triso_atops_fork`), which supplies the fission-product
//! release physics that `sembawang` orchestrates.

// Rustdoc without source pages (teaching site, gh:#518): lessons show
// anchored snippets and link to GitHub at the build commit instead.
#![doc(html_no_source)]

// `no_std` + `alloc` without the default `std` feature (gh:#1634, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports, unused_variables, unused_mut, irrefutable_let_patterns))]

// `alloc` by name (a std crate does not put `alloc` in its extern prelude on
// its own).
extern crate alloc;
extern crate anyhow_nostd as anyhow;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}

/// The wall clock (gh:#1634). With `std` (and in tests) this is std's. Without
/// it there is no clock: `SystemTime::now()` is a fixed instant, `elapsed()`
/// and `duration_since()` are zero, so a loop timer reads zero and a
/// clock-derived seed is a fixed one (see `DiffusionRandomCache::new`).
pub(crate) mod time {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

    /// Stand-in for `std::time::SystemTime` without std: no clock.
    #[cfg(not(any(feature = "std", test)))]
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct SystemTime;

    /// Stand-in for `std::time::UNIX_EPOCH` without std.
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) const UNIX_EPOCH: SystemTime = SystemTime;

    #[cfg(not(any(feature = "std", test)))]
    impl SystemTime {
        /// A fixed instant: there is no clock to read.
        pub(crate) fn now() -> Self {
            SystemTime
        }
        /// Always zero without a clock.
        pub(crate) fn elapsed(&self) -> Result<core::time::Duration, ()> {
            Ok(core::time::Duration::ZERO)
        }
        /// Always zero without a clock.
        pub(crate) fn duration_since(&self, _earlier: SystemTime) -> Result<core::time::Duration, ()> {
            Ok(core::time::Duration::ZERO)
        }
    }
}

/// One-time initialisation (gh:#1634): std's `OnceLock` with `std` (and in
/// tests), a `spin::Once` wrapper with the same two methods without it.
pub(crate) mod sync {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::sync::OnceLock;

    /// `OnceLock` without std, on `spin::Once`.
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) struct OnceLock<T>(spin::Once<T>);

    #[cfg(not(any(feature = "std", test)))]
    impl<T> OnceLock<T> {
        /// An empty cell.
        pub(crate) const fn new() -> Self {
            OnceLock(spin::Once::new())
        }
        /// The value, initialising it with `f` on first use.
        pub(crate) fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
            self.0.call_once(f)
        }
    }
}

/// prelude is here for easy imports
pub mod prelude;

/// **A copy of RustBCA, not boon-lay's own code** — the binary-collision
/// Monte Carlo for ions and the recoils they knock on (fission fragments,
/// alphas, recoil knock-ons), copied from `lcpp-org/RustBCA` ~~v3.0.0~~
/// (`a356280d`, which is v3.0.0 plus 17 commits, gh:#1426; GPL-3.0, Jon Drobny and Davide Curreli) and made `no_std` +
/// `alloc`. boon-lay re-exports it here (gh:#876, item 7). The module docs
/// carry the attribution and every local change; fragment results are
/// illustrative until V&V'd.
pub mod rustbca;

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
/// `std` only (gh:#1634): parses OpenMC's XML (`serde-xml-rs`).
#[cfg(feature = "std")]
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
#[cfg(all(feature = "std", not(target_os = "android"), not(target_arch = "wasm32")))]
pub mod gpu;

/// Eulerian / continuum-diffusion TRISO fission-product release — a Rust fork of
/// Idaho National Laboratory's TRISO-ATOPS (MIT). This is the continuum
/// complement to the crate's Lagrangian (single-atom Monte-Carlo) model: it uses
/// closed-form analytical solutions to the Fickian diffusion equation (Booth,
/// breakthrough, graphite-attenuation models) to predict per-nuclide release
/// fractions. See `docs/triso-atops-fork.md` and the module-level docs.
pub mod triso_atops_fork;

/// **Extensions of the TRISO-ATOPS model — NOT a port.** boon-lay's own
/// additions built on [`triso_atops_fork`] without modifying it: removal rates
/// per transport group or per element instead of one `k_plate` / `k_clean` per
/// reactor (gh:#583). Each reduces exactly to the port when given upstream's
/// inputs. See the module docs.
pub mod triso_atops_extensions;

/// **boon-lay fuel failure** — TRISO coated-particle failure fractions. This
/// is boon-lay's own model: a Rust implementation of the *formulas* published
/// in the PANAMA-I report (Verfondern & Nabielek, Jülich HTA-IB-03/90, 1990),
/// coded agentically by an AI coding agent. It is **not** the PANAMA code, whose
/// source this project does not have; see the module docs on naming. Supplies what
/// [`triso_atops_fork`]'s `FailureFractions` currently takes as a
/// hand-entered number. See the module docs for what is implemented and what
/// is deliberately left as an input.
pub mod fuel_failure;

/// Cited chemical-attack rate laws (graphite-steam oxidation, kernel
/// hydrolysis) for the water-ingress source term (gh:#401).
pub mod chemistry;

/// Serial stand-ins for the `rayon` surface this crate uses, on `wasm32` where
/// `rayon` does not build, and without `std` (gh:#1634). See the module docs.
#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
mod wasm_par;
