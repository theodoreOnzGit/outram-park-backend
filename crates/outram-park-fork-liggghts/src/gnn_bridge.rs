//! Bridge to RAFFLES's graph-neural-network layer: the contact graph, and how
//! much message-passing reach a DEM surrogate needs.
//!
//! Available only with this crate's `gnn` feature, which is **off by default**
//! — a DEM user who is not building a surrogate should not compile a tensor
//! library.
//!
//! # Why a DEM code wants this
//!
//! A granular assembly is already a graph: particles are nodes and contacts
//! are edges. That makes it a natural target for a message-passing surrogate —
//! and it makes the *reach* question immediate and physical, because force is
//! transmitted along **force chains** that span many particles.
//!
//! The question a surrogate builder has to answer before training anything is
//! how many message-passing steps the physics needs. Guess low and the network
//! under-reaches: a particle's predicted force cannot depend on a particle
//! further away than the step count, so a force chain longer than that cannot
//! be represented at all, however long the model trains.
//! [`reach_bound`] answers it from the material and the timestep, with no
//! training and no neural network involved.
//!
//! # A DEM step is hyperbolic
//!
//! Contact forces propagate at the elastic wave speed of the solid, which is
//! finite, so the bound is the CFL-style one: the message reach `M * h` must
//! cover the distance `c * dt` a stress wave travels in one step. For a
//! packing of particles of diameter `d`, one message hop covers roughly one
//! particle diameter, so
//!
//! ```text
//! M >= c * dt / d,   c = sqrt(E / rho)
//! ```
//!
//! That is usually a small number for a well-chosen DEM timestep — which is
//! itself limited by the Rayleigh criterion for the same physical reason — and
//! that is the useful result: **a DEM surrogate does not need a deep network,
//! and a paper reporting one should say why.**
//!
//! # What this module is not
//!
//! It builds graphs and computes bounds. It does not train, own or evaluate a
//! surrogate: that belongs to the caller, using
//! [`raffles::gnn::MessagePassingNet`] and `raffles::gnn::training`. Nothing
//! here changes how the DEM solver runs.

use raffles::gnn::bound::{physics_guided_lower_bound, IterationBound, PdeClass};
use raffles::gnn::Graph;

use crate::particle::Particle;
use crate::DemError;

/// Builds the contact graph of a particle assembly.
///
/// Two particles are connected when the gap between their surfaces is at most
/// `skin`. With `skin = 0` that is exactly the set of particles currently in
/// contact; a positive skin is the usual neighbour-list margin, and is what you
/// want if the graph will be reused for more than one timestep.
///
/// `skin` is in metres, like every length in this crate.
///
/// # Errors
///
/// [`DemError::InvalidInput`] if `particles` is empty or `skin` is negative.
/// The underlying graph builder also rejects a non-finite coordinate, which
/// cannot happen for particles built through [`Particle::new`] but can after a
/// diverged integration — and catching it here is better than training on it.
pub fn contact_graph(particles: &[Particle], skin: f64) -> Result<Graph, DemError> {
    if particles.is_empty() {
        return Err(DemError::InvalidInput(
            "a contact graph needs at least one particle".to_string(),
        ));
    }
    let centres: Vec<Vec<f64>> = particles
        .iter()
        .map(|p| vec![p.position.x, p.position.y, p.position.z])
        .collect();
    let radii: Vec<f64> = particles.iter().map(|p| p.radius).collect();
    Graph::contact_graph(&centres, &radii, skin).map_err(|e| {
        DemError::InvalidInput(format!("could not build the contact graph: {e}"))
    })
}

/// The message-passing reach a surrogate of this assembly needs, for a given
/// material and timestep.
///
/// - `particles` — the assembly, used for its contact graph and its mean
///   particle diameter (the distance one message hop covers).
/// - `youngs_modulus` — `E` in pascals, as [`crate::contact::HertzContact`]
///   stores it.
/// - `density` — solid density in kg/m³. Note this is the **solid** density,
///   not the bulk density of the packing: the stress wave travels through the
///   material, not through the voids.
/// - `time_step` — the DEM timestep in seconds.
/// - `message_passing_steps` — what a candidate model is configured with, if
///   there is one; pass `None` to ask only what is required.
///
/// # Errors
///
/// [`DemError::InvalidInput`] if the assembly is empty, if `youngs_modulus`,
/// `density` or `time_step` is not strictly positive, or if the mean particle
/// diameter is not positive.
pub fn reach_bound(
    particles: &[Particle],
    youngs_modulus: f64,
    density: f64,
    time_step: f64,
    message_passing_steps: Option<usize>,
) -> Result<IterationBound, DemError> {
    if !(youngs_modulus > 0.0) || !youngs_modulus.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "Young's modulus must be finite and strictly positive, got {youngs_modulus}"
        )));
    }
    if !(density > 0.0) || !density.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "density must be finite and strictly positive, got {density}"
        )));
    }
    if !(time_step > 0.0) || !time_step.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "the timestep must be finite and strictly positive, got {time_step}"
        )));
    }

    let graph = contact_graph(particles, 0.0)?;
    let mean_diameter =
        2.0 * particles.iter().map(|p| p.radius).sum::<f64>() / particles.len() as f64;
    if !(mean_diameter > 0.0) {
        return Err(DemError::InvalidInput(
            "the mean particle diameter is not positive".to_string(),
        ));
    }

    // Elastic (bar) wave speed. The bulk or shear speed differs by an O(1)
    // factor depending on the mode; sqrt(E / rho) is the standard choice in
    // the DEM timestep literature (it is what the Rayleigh timestep uses), and
    // being consistent with that is worth more here than a mode-resolved
    // refinement, since the bound is rounded up to an integer anyway.
    let signal_speed = (youngs_modulus / density).sqrt();

    physics_guided_lower_bound(
        PdeClass::Hyperbolic {
            signal_speed,
            time_step,
        },
        &graph,
        mean_diameter,
        message_passing_steps,
    )
    .map_err(|e| DemError::InvalidInput(format!("could not compute the reach bound: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particle::Vec3;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    /// A chain of `n` touching unit-radius spheres along x.
    fn chain(n: usize, radius: f64) -> Vec<Particle> {
        (0..n)
            .map(|i| {
                Particle::new(
                    Vec3 {
                        x: i as f64 * 2.0 * radius,
                        y: 0.0,
                        z: 0.0,
                    },
                    Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    Mass::new::<kilogram>(1.0),
                    Length::new::<meter>(radius),
                    ThermodynamicTemperature::new::<kelvin>(300.0),
                )
                .unwrap()
            })
            .collect()
    }

    /// **Methodology.** A chain of touching equal spheres must give a path
    /// graph: `n - 1` undirected contacts, so `2(n - 1)` directed edges, and a
    /// diameter of `n - 1` hops. Checked for 6 spheres of radius 0.5 m placed
    /// exactly one diameter apart.
    ///
    /// **Result** (2026-09-16): 10 directed edges and diameter 5, both exact.
    #[test]
    fn a_touching_chain_gives_a_path_graph() {
        let particles = chain(6, 0.5);
        let graph = contact_graph(&particles, 0.0).unwrap();
        assert_eq!(graph.node_count(), 6);
        assert_eq!(graph.edge_count(), 10);
        assert_eq!(graph.diameter(), 5);
        assert!(graph.is_connected());
    }

    /// **Methodology — the polydisperse case, which a single-radius graph gets
    /// wrong.** Two spheres of radius 0.5 m and 1.5 m whose centres are 1.9 m
    /// apart are in contact (0.5 + 1.5 = 2.0 > 1.9), but a radius graph using
    /// either particle's own radius as the cutoff would miss them. The contact
    /// graph must find the edge.
    ///
    /// **Result** (2026-09-16): the contact is found; a uniform 1.0 m cutoff
    /// does not find it.
    #[test]
    fn polydisperse_contacts_need_per_particle_radii() {
        let mut particles = chain(1, 0.5);
        particles.push(
            Particle::new(
                Vec3 {
                    x: 1.9,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Mass::new::<kilogram>(1.0),
                Length::new::<meter>(1.5),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap(),
        );
        let graph = contact_graph(&particles, 0.0).unwrap();
        assert_eq!(graph.edge_count(), 2, "the contact should be found");

        // A uniform-radius graph at the smaller particle's own scale misses it.
        let centres: Vec<Vec<f64>> = particles
            .iter()
            .map(|p| vec![p.position.x, p.position.y, p.position.z])
            .collect();
        let uniform = Graph::radius_graph(&centres, 1.0).unwrap();
        assert_eq!(uniform.edge_count(), 0, "this is the failure being avoided");
    }

    /// **Methodology — the bound, against a hand-computed value.** Steel-like
    /// properties: `E = 2.0e11 Pa`, `rho = 7800 kg/m^3`, giving an elastic wave
    /// speed of `sqrt(2e11 / 7800) = 5063.7 m/s`. With 1 mm particles
    /// (diameter 1e-3 m) and a timestep of 1e-7 s, the wave travels
    /// `5.064e-4 m` per step, which is 0.506 particle diameters — so a single
    /// message-passing step suffices and the bound is 1.
    ///
    /// Raising the timestep to 1e-6 s gives 5.06 diameters and a bound of 6.
    ///
    /// **Result** (2026-09-16): 1 and 6, as computed by hand.
    #[test]
    fn the_reach_bound_matches_a_hand_computed_cfl_number() {
        let particles = chain(8, 0.5e-3);
        let bound = reach_bound(&particles, 2.0e11, 7800.0, 1.0e-7, None).unwrap();
        println!("DEM reach bound at dt = 1e-7 s: {}", bound.rationale);
        assert_eq!(bound.required, 1);

        let coarser = reach_bound(&particles, 2.0e11, 7800.0, 1.0e-6, None).unwrap();
        println!("DEM reach bound at dt = 1e-6 s: {}", coarser.rationale);
        assert_eq!(coarser.required, 6);
    }

    /// **Methodology.** A configured model must be checked against the bound,
    /// with the shortfall quantified. A 2-step model against a problem needing
    /// 6.
    ///
    /// **Result** (2026-09-16): not satisfied, shortfall 4; an 8-step model
    /// passes.
    #[test]
    fn an_under_reaching_dem_surrogate_is_caught() {
        let particles = chain(8, 0.5e-3);
        let under = reach_bound(&particles, 2.0e11, 7800.0, 1.0e-6, Some(2)).unwrap();
        assert_eq!(under.is_satisfied(), Some(false));
        assert_eq!(under.shortfall(), Some(4));

        let over = reach_bound(&particles, 2.0e11, 7800.0, 1.0e-6, Some(8)).unwrap();
        assert_eq!(over.is_satisfied(), Some(true));
    }

    /// **Methodology.** Malformed input must be refused: an empty assembly, a
    /// negative skin, and non-positive material properties or timestep.
    ///
    /// **Result.** All five rejected (2026-09-16).
    #[test]
    fn malformed_bridge_requests_are_refused() {
        let particles = chain(3, 0.5);
        assert!(contact_graph(&[], 0.0).is_err());
        assert!(contact_graph(&particles, -1.0).is_err());
        assert!(reach_bound(&particles, 0.0, 7800.0, 1e-6, None).is_err());
        assert!(reach_bound(&particles, 2.0e11, 0.0, 1e-6, None).is_err());
        assert!(reach_bound(&particles, 2.0e11, 7800.0, 0.0, None).is_err());
    }
}
