/// Reaction types and secondary particle sampling.
///
/// C++ source: `src/reaction.cpp` (424 LOC), `include/openmc/reaction.h`.
/// Also: `src/physics_common.cpp` — secondary product angle/energy sampling.
///
/// OpenMC models each reaction as a `Reaction` object that stores:
///   - MT number (ENDF reaction designation, e.g. MT=2 elastic, MT=18 fission)
///   - Q-value (energy release)
///   - Secondary product distributions (angle + energy)
///
/// The `Reaction` trait here mirrors the C++ virtual base.

/// ENDF reaction MT number (subset relevant to neutron transport).
/// MT values are stored as associated constants for documentation; the enum
/// itself does not carry integer discriminants to allow the `Other(u32)` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactionMT {
    Elastic,    // MT 2
    Fission,    // MT 18
    Capture,    // MT 102
    Inelastic,  // MT 51 (first excited level; 51–90 are discrete inelastic)
    N2N,        // MT 16
    N3N,        // MT 17
    Total,      // MT 1
    Other(u32),
}

impl ReactionMT {
    /// ENDF MT integer for this reaction.
    pub fn mt_number(self) -> u32 {
        match self {
            Self::Total     => 1,
            Self::Elastic   => 2,
            Self::N2N       => 16,
            Self::N3N       => 17,
            Self::Fission   => 18,
            Self::Inelastic => 51,
            Self::Capture   => 102,
            Self::Other(n)  => n,
        }
    }
}

/// Interface for a single nuclear reaction.  Maps to the virtual `Reaction` base.
pub trait Reaction {
    fn mt(&self) -> ReactionMT;
    fn q_value(&self) -> f64;

    /// Sample secondary neutron state post-reaction.
    ///
    /// Returns `(energy_out_eV, direction_cosines)`.
    /// TODO: port secondary sampling from `physics_common.cpp`.
    fn sample_secondary(
        &self,
        e_in: f64,
        seed: &mut u64,
    ) -> Option<(f64, (f64, f64, f64))>;
}
