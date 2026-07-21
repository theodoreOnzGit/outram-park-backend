//! Unified "which material is at this point?" query over every random-media model.
//!
//! A stochastic (random) medium is one whose geometry is not known deterministically:
//! dispersion fuel, TRISO kernels in a graphite matrix, burnable-poison particles in
//! an absorber. Three families of model answer the transport loop's material query,
//! trading memory against fidelity:
//!
//! - **Explicit / RSA** ([`RsaMedium`]) — every inclusion is generated and stored, so
//!   the point-membership answer is *exact*. High fidelity, high memory. This is the
//!   **reference solution** every approximate model is judged against. Built on the
//!   already-implemented [`crate::pebble_beds::sphere_packing::pack_spheres`].
//! - **Chord Length Sampling (CLS)** ([`super::cls::ClsMedium`]) — nothing is stored;
//!   inclusion crossings are re-sampled from a chord-length distribution on the fly.
//!   Cheap, but *memoryless*: a neutron that turns around does not see the inclusion
//!   it just crossed.
//! - **Semi-Implicit CLS (SCLS)** ([`super::scls::SclsMedium`]) — a bounded window of
//!   recent inclusions is retained, recovering the local geometric memory CLS discards
//!   while keeping memory bounded. The primary research target of this module.
//!
//! # Design-doc deviations (workspace rules take precedence)
//!
//! The v0.1 design scaffold specifies `pub trait StochasticMedium { fn material_at(…) }`
//! and dispatches through it. The workspace Rust design rules forbid trait-object
//! dispatch, so this module instead exposes the closed enum [`StochasticMedium`]
//! dispatched by `match`. The set of random-media models is closed and known at compile
//! time, which is exactly the case the enum rule is written for: adding a model forces
//! every `match` site to handle it, and no variant is heap-allocated.
//!
//! The doc's `material_at(&self, position, neutron: &Neutron)` is also adjusted:
//!
//! - It takes **`&mut self`**, because CLS and SCLS mutate sampler/history state on
//!   every query — they are not pure functions of position.
//! - It takes **`seed: &mut u64`** rather than a whole particle, matching the crate's
//!   existing LCG convention (see
//!   [`crate::pebble_beds::delta_tracking::sample_delta_distance`]).
//!   The query needs randomness, not the full phase-space state.
//! - It returns a [`Result`], so a not-yet-implemented model reports that instead of
//!   panicking inside a transport loop.
//!
//! # A caveat worth stating plainly
//!
//! Point membership is only a *well-posed* question for an explicit medium. CLS and
//! SCLS are **flight-level** samplers: they answer "where is the next inclusion
//! boundary along this ray?", and reconstruct material occupancy statistically rather
//! than storing it. [`StochasticMedium::material_at`] is therefore exact for
//! [`StochasticMedium::Rsa`] and approximate-by-construction for the others. Treat a
//! CLS/SCLS material answer as a sample, not as ground truth.

use crate::geometry::position::Position;
use crate::pebble_beds::sphere_packing::PackedSpheres;

/// Index of a material in the caller's material table.
///
/// A transparent newtype over `usize` rather than a bare integer, so a material index
/// cannot be silently swapped with a cell index, a nuclide index, or a sphere index.
/// This crate does not own the material table; the caller maps an id back to its
/// [`crate::material::material::Material`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialId(pub usize);

impl MaterialId {
    /// The raw table index.
    pub fn index(self) -> usize {
        self.0
    }
}

/// Errors from a stochastic-medium material query.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MediumError {
    /// The model is scaffolded but its sampling logic is not built out yet.
    ///
    /// Carries the model name so a transport driver can report which one it hit.
    #[error("stochastic medium model '{0}' is scaffolded but not implemented yet")]
    NotImplemented(&'static str),
}

/// Explicit random medium — every inclusion stored, point membership exact.
///
/// Wraps the already-implemented RSA packing ([`crate::pebble_beds::sphere_packing::PackedSpheres`])
/// and tags the two phases with material ids. This is the reference model: CLS and SCLS
/// are correct to the extent they reproduce what this returns.
#[derive(Debug, Clone)]
pub struct RsaMedium {
    packing: PackedSpheres,
    inclusion: MaterialId,
    matrix: MaterialId,
}

impl RsaMedium {
    /// Build an explicit medium from a packing and the two phase materials.
    ///
    /// - `packing` — the generated inclusion set (e.g. TRISO kernels).
    /// - `inclusion` — material inside a packed sphere (e.g. UO2 kernel).
    /// - `matrix` — material everywhere else (e.g. graphite).
    pub fn new(packing: PackedSpheres, inclusion: MaterialId, matrix: MaterialId) -> Self {
        Self {
            packing,
            inclusion,
            matrix,
        }
    }

    /// Exact point membership: inclusion material if `p` is inside a packed sphere,
    /// matrix material otherwise.
    ///
    /// `p` is in cm, in the packing's own frame (the cube centred on the origin).
    pub fn material_at(&self, p: Position) -> MaterialId {
        if self.packing.is_inside_kernel(p) {
            self.inclusion
        } else {
            self.matrix
        }
    }

    /// The underlying packing (inclusion centres and radii).
    pub fn packing(&self) -> &PackedSpheres {
        &self.packing
    }

    /// Material id of the inclusion phase.
    pub fn inclusion_material(&self) -> MaterialId {
        self.inclusion
    }

    /// Material id of the matrix phase.
    pub fn matrix_material(&self) -> MaterialId {
        self.matrix
    }
}

/// A random-media model, dispatched by `match` (no trait objects — see module docs).
///
/// Ordered by increasing approximation: [`Self::Rsa`] is exact and expensive,
/// [`Self::Scls`] retains bounded memory, [`Self::Cls`] retains none.
#[derive(Debug, Clone)]
pub enum StochasticMedium {
    /// Explicit stored geometry — the reference solution. Implemented.
    Rsa(RsaMedium),
    /// Memoryless chord-length sampling. Scaffolded — see [`super::cls`].
    Cls(super::cls::ClsMedium),
    /// Semi-implicit CLS with retained histories. Scaffolded — see [`super::scls`].
    Scls(super::scls::SclsMedium),
}

impl StochasticMedium {
    /// Which material occupies `position`.
    ///
    /// Exact for [`Self::Rsa`]; a *statistical reconstruction* for [`Self::Cls`] and
    /// [`Self::Scls`] (see the module-level caveat). `seed` is the caller's LCG stream,
    /// advanced in place by the models that need randomness.
    ///
    /// # Errors
    /// [`MediumError::NotImplemented`] for models whose sampling is not built out yet.
    pub fn material_at(
        &mut self,
        position: Position,
        seed: &mut u64,
    ) -> Result<MaterialId, MediumError> {
        match self {
            Self::Rsa(m) => Ok(m.material_at(position)),
            Self::Cls(m) => m.material_at(position, seed),
            Self::Scls(m) => m.material_at(position, seed),
        }
    }

    /// Short model name, for error messages and benchmark tables.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rsa(_) => "RSA (explicit)",
            Self::Cls(_) => "CLS",
            Self::Scls(_) => "SCLS",
        }
    }

    /// Whether this model answers [`Self::material_at`] exactly.
    ///
    /// Only the explicit model does. Benchmarks use this to pick the reference arm.
    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Rsa(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pebble_beds::sphere_packing::PackingConfig;
    use crate::pebble_beds::sphere_packing::PackingMethod;

    /// An explicit RSA medium reports inclusion material inside a packed sphere and
    /// matrix material outside it.
    #[test]
    fn rsa_medium_point_membership_matches_packing() {
        let cfg = PackingConfig {
            particle_radius: 0.05,
            packing_fraction: 0.1,
            domain_half_width: 0.5,
            method: PackingMethod::Rsa,
            seed: 1,
        };
        let spheres = cfg
            .generate()
            .expect("RSA packing at pf=0.1 should succeed");
        assert!(!spheres.is_empty(), "expected a non-empty packing");

        let packing =
            PackedSpheres::from_spheres(spheres, cfg.domain_half_width, cfg.particle_radius);
        let first_centre = packing.spheres()[0].center;

        let medium = RsaMedium::new(packing, MaterialId(7), MaterialId(3));

        // The centre of a packed sphere is unambiguously inclusion material.
        assert_eq!(medium.material_at(first_centre), MaterialId(7));

        // A corner of the cube cannot hold a centre (centres stay >= radius from the
        // wall), so it is matrix.
        let corner = Position::new(0.499, 0.499, 0.499);
        assert_eq!(medium.material_at(corner), MaterialId(3));
    }

    /// The enum reports exactness correctly — benchmarks rely on this to choose the
    /// reference arm.
    #[test]
    fn only_explicit_model_is_exact() {
        let packing = PackedSpheres::from_spheres(Vec::new(), 1.0, 0.1);
        let rsa = StochasticMedium::Rsa(RsaMedium::new(packing, MaterialId(0), MaterialId(1)));
        assert!(rsa.is_exact());
        assert_eq!(rsa.name(), "RSA (explicit)");
    }
}
