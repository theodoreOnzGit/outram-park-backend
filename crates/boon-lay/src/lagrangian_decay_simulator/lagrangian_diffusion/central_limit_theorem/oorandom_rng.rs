use openmc_libs::rng::lcg::{prn, MULT, INC};

/// Stateful RNG adapter for the diffusion simulators.
///
/// Previously wrapped `oorandom::Rand64` and implemented `rand_core::RngCore`.
/// Now wraps the OpenMC LCG `u64` state directly; `rand_core` is no longer a
/// dependency.  The inner state is public (`pub .0`) so that call sites in
/// `single_particle_simulator/mod.rs` can pass `&mut rng.0` directly to the
/// `seed: &mut u64` samplers in `isotropic_scattering` and `central_limit_theorem`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OoRng64(pub u64);

impl OoRng64 {
    /// Create from a 128-bit seed (same signature as the old `oorandom::Rand64::new`).
    pub fn from_u128(seed: u128) -> Self {
        let mut state = seed as u64;
        state = state.wrapping_mul(MULT).wrapping_add(INC);
        OoRng64(state)
    }

    /// Create from a 64-bit seed.
    pub fn from_u64(seed: u64) -> Self {
        Self::from_u128(seed as u128)
    }

    /// Return a uniform float in [0, 1) and advance the state.
    #[inline]
    pub fn rand_float(&mut self) -> f64 {
        prn(&mut self.0)
    }

    /// Advance the state and return the raw 64-bit word.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(MULT).wrapping_add(INC);
        self.0
    }
}
