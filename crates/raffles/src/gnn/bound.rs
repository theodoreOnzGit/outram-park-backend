//! The physics-guided lower bound on message-passing iterations.
//!
//! # The problem: under-reaching
//!
//! A message-passing graph network moves information one hop per iteration. Run
//! `M` iterations and a node's prediction can depend only on nodes within `M`
//! hops of it — no further, ever, at any amount of training. If the physics
//! says a node's next state depends on something further away than that, the
//! network is structurally unable to represent the answer. It will train, the
//! loss will fall, and the rollout will be wrong.
//!
//! Tesan and Iparraguirre (2025) name this **under-reaching** and make the
//! point that `M` is therefore not a hyperparameter to tune — it is a quantity
//! the PDE and the mesh *determine*, in the same way and for the same reason
//! that the CFL condition determines a stable explicit time step.
//!
//! # The two bounds
//!
//! [`PdeClass`] chooses which applies, because the two families of PDE
//! propagate information in categorically different ways:
//!
//! - **Hyperbolic** (waves, advection, elastic contact). Information travels at
//!   a finite speed `c`. Over one integration step it covers `c * dt`, so the
//!   network's reach `M * h` — `M` hops of characteristic edge length `h` —
//!   must cover it:
//!
//!   ```text
//!   M >= c * dt / h
//!   ```
//!
//!   which is the CFL number, read as a requirement on message passing rather
//!   than on the time step.
//!
//! - **Parabolic and elliptic** (diffusion, heat conduction, Poisson). The
//!   governing equation propagates information across the whole domain
//!   instantaneously — a Poisson solve has no finite signal speed at all — so
//!   the only sufficient reach is the entire graph:
//!
//!   ```text
//!   M >= graph diameter in hops
//!   ```
//!
//! # Provenance of the formulas
//!
//! The *idea* — that these bounds exist and that they explain observed rollout
//! failures — is Tesan and Iparraguirre's, and their repository is GPL-3.0, so
//! a code-level port would be permitted. The two expressions above are not
//! transcribed from that repository, because they do not appear in it: the
//! published code fixes its iteration count by configuration and the bounds
//! live in the paper. They are derived here from the principles the paper's
//! abstract states, and each derivation is written out above so a reader can
//! check the reasoning rather than trust a constant.
//!
//! **This matters for how much weight to put on the numbers.** Where the paper
//! gives a sharper constant — a safety factor, a different characteristic
//! length, a correction for the encoder's own reach — this module's bound will
//! differ from it. Treat these as the elementary reach requirement, which is
//! necessary, rather than as a reproduction of the paper's precise statement.
//!
//! # Reference
//!
//! - L. Tesan and M. M. Iparraguirre et al. (2025). On the under-reaching
//!   phenomenon in message passing neural PDE solvers: revisiting the CFL
//!   condition. arXiv:2507.08861.
//!   <https://arxiv.org/abs/2507.08861>

use super::graph::Graph;
use crate::{RafflesError, Result};

/// Which family a PDE belongs to, and therefore how information propagates
/// through it.
///
/// The classification is the standard one for second-order PDEs and it is the
/// only thing the bound needs to know about the physics — which is what makes
/// the bound usable before any training has been done.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdeClass {
    /// Finite propagation speed: waves, advection, elastic contact,
    /// compressible flow. Information travels along characteristics at speed
    /// `c`.
    Hyperbolic {
        /// Characteristic signal speed, in the mesh's length unit per unit of
        /// the integration step's time unit. For a wave equation this is the
        /// wave speed; for advection, the advection velocity; for elastic
        /// contact, the sound speed in the material.
        signal_speed: f64,
        /// The integration time step the network advances per forward pass, in
        /// the same time unit.
        time_step: f64,
    },
    /// Infinite propagation speed: heat conduction, diffusion. A disturbance
    /// anywhere changes the solution everywhere immediately, however slightly.
    Parabolic,
    /// No time derivative at all: Poisson, steady-state elasticity. The
    /// solution at a point depends on the whole domain's boundary conditions
    /// simultaneously.
    Elliptic,
}

impl PdeClass {
    /// Whether the class propagates information at a finite speed.
    ///
    /// The whole reason the two bounds differ, so it is worth being able to ask
    /// directly rather than by matching on the variant.
    pub fn has_finite_signal_speed(&self) -> bool {
        matches!(self, Self::Hyperbolic { .. })
    }
}

/// The verdict on a chosen number of message-passing iterations.
#[derive(Debug, Clone, PartialEq)]
pub struct IterationBound {
    /// The smallest number of message-passing iterations the physics admits.
    pub required: usize,
    /// What the model is actually configured to use, if the caller supplied it.
    pub configured: Option<usize>,
    /// Physical distance one message hop covers, in the mesh's length unit.
    pub hop_length: f64,
    /// Physical distance the configured number of iterations reaches, if known.
    pub reach: Option<f64>,
    /// Distance the physics requires be reached in one integration step, for a
    /// hyperbolic problem; `None` for the classes with infinite signal speed,
    /// where the requirement is topological rather than metric.
    pub required_distance: Option<f64>,
    /// Which rule produced `required`, in words, for a report or a log line.
    pub rationale: String,
}

impl IterationBound {
    /// Whether the configured iteration count satisfies the bound.
    ///
    /// `None` when no configuration was supplied — deliberately not `false`,
    /// since "not checked" and "failed" are different states and collapsing
    /// them is how an unchecked model gets described as validated.
    pub fn is_satisfied(&self) -> Option<bool> {
        self.configured.map(|m| m >= self.required)
    }

    /// How many iterations are missing, or zero when the bound is met.
    pub fn shortfall(&self) -> Option<usize> {
        self.configured.map(|m| self.required.saturating_sub(m))
    }
}

/// The physics-guided lower bound on message-passing iterations, for a given
/// PDE class and mesh.
///
/// `hop_length` is the physical distance one message hop covers — in a radius
/// graph, the connectivity radius; in a mesh graph, the characteristic edge
/// length. Using the *mean* edge length rather than the shortest is optimistic:
/// the bound is only as good as the worst hop along the path the information
/// has to travel, so pass the smaller number when in doubt.
///
/// `configured` is the model's chosen iteration count, if there is one; passing
/// it fills in [`IterationBound::is_satisfied`].
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `hop_length` is not strictly positive,
/// if a hyperbolic class has a non-positive signal speed or time step, or if
/// the graph is disconnected and the class needs its diameter — an
/// infinite-speed problem on a disconnected mesh has *no* sufficient iteration
/// count, and reporting a finite one would be wrong rather than conservative.
pub fn physics_guided_lower_bound(
    class: PdeClass,
    graph: &Graph,
    hop_length: f64,
    configured: Option<usize>,
) -> Result<IterationBound> {
    if !(hop_length > 0.0) || !hop_length.is_finite() {
        return Err(RafflesError::InvalidParameter {
            parameter: "hop_length".to_string(),
            value: hop_length,
            reason: "one message hop must cover a strictly positive distance".to_string(),
        });
    }

    let (required, required_distance, rationale) = match class {
        PdeClass::Hyperbolic {
            signal_speed,
            time_step,
        } => {
            if !(signal_speed > 0.0) || !signal_speed.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "signal_speed".to_string(),
                    value: signal_speed,
                    reason: "a hyperbolic problem needs a strictly positive signal speed"
                        .to_string(),
                });
            }
            if !(time_step > 0.0) || !time_step.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "time_step".to_string(),
                    value: time_step,
                    reason: "the integration step must be strictly positive".to_string(),
                });
            }
            let distance = signal_speed * time_step;
            let exact = distance / hop_length;
            // Ceiling, and never zero: even a signal that travels less than one
            // hop per step needs one iteration to move at all.
            let required = (exact.ceil() as usize).max(1);
            (
                required,
                Some(distance),
                format!(
                    "hyperbolic: a signal at {signal_speed} per unit time covers {distance} in one \
                     step of {time_step}, which takes {exact:.4} hops of {hop_length}; rounded up \
                     to {required}"
                ),
            )
        }
        PdeClass::Parabolic | PdeClass::Elliptic => {
            if !graph.is_connected() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "graph".to_string(),
                    value: graph.node_count() as f64,
                    reason: "an infinite-speed problem on a disconnected mesh has no sufficient \
                             iteration count: parts of the domain can never influence each other"
                        .to_string(),
                });
            }
            let diameter = graph.diameter();
            let name = if matches!(class, PdeClass::Parabolic) {
                "parabolic"
            } else {
                "elliptic"
            };
            (
                diameter.max(1),
                None,
                format!(
                    "{name}: propagation is instantaneous, so one step must reach the whole \
                     domain; the graph diameter is {diameter} hops"
                ),
            )
        }
    };

    Ok(IterationBound {
        required,
        configured,
        hop_length,
        reach: configured.map(|m| m as f64 * hop_length),
        required_distance,
        rationale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 9x9 lattice of unit extent, connected at just over its spacing.
    fn unit_lattice() -> (Graph, f64) {
        let spacing = 1.0 / 8.0;
        let mut points = Vec::new();
        for i in 0..9 {
            for j in 0..9 {
                points.push(vec![i as f64 * spacing, j as f64 * spacing]);
            }
        }
        let radius = spacing * 1.01;
        (Graph::radius_graph(&points, radius).unwrap(), radius)
    }

    /// **Methodology — the CFL reading.** On the unit lattice with hop length
    /// 0.12625, a wave at speed 1.0 over a step of 0.05 covers 0.05, which is
    /// 0.396 hops — so a single iteration suffices and the bound must be 1
    /// (never 0: a network that passes no messages cannot move information at
    /// all). Raising the step to 0.5 gives 3.96 hops and a bound of 4.
    ///
    /// **Result** (2026-09-16): 1 and 4, as derived.
    #[test]
    fn the_hyperbolic_bound_is_the_cfl_number_rounded_up() {
        let (graph, radius) = unit_lattice();
        let small = physics_guided_lower_bound(
            PdeClass::Hyperbolic {
                signal_speed: 1.0,
                time_step: 0.05,
            },
            &graph,
            radius,
            None,
        )
        .unwrap();
        assert_eq!(small.required, 1, "{}", small.rationale);

        let large = physics_guided_lower_bound(
            PdeClass::Hyperbolic {
                signal_speed: 1.0,
                time_step: 0.5,
            },
            &graph,
            radius,
            None,
        )
        .unwrap();
        println!("hyperbolic bound: {}", large.rationale);
        assert_eq!(large.required, 4);
        assert!((large.required_distance.unwrap() - 0.5).abs() < 1e-12);
    }

    /// **Methodology — the elliptic reading.** On the same lattice, an elliptic
    /// problem must reach the whole domain in one step, so the bound is the
    /// graph diameter: 8 across plus 8 down = 16 hops for a 4-neighbour 9x9
    /// lattice.
    ///
    /// **Result** (2026-09-16): 16, matching the diameter exactly; the
    /// parabolic class gives the same number, as it must.
    #[test]
    fn the_elliptic_bound_is_the_graph_diameter() {
        let (graph, radius) = unit_lattice();
        assert_eq!(graph.diameter(), 16);

        let elliptic =
            physics_guided_lower_bound(PdeClass::Elliptic, &graph, radius, None).unwrap();
        let parabolic =
            physics_guided_lower_bound(PdeClass::Parabolic, &graph, radius, None).unwrap();
        println!("elliptic bound: {}", elliptic.rationale);
        assert_eq!(elliptic.required, 16);
        assert_eq!(parabolic.required, 16);
    }

    /// **Methodology — the failure the bound exists to catch.** A model
    /// configured with 4 message-passing iterations on an elliptic problem
    /// needing 16 must be reported as under-reaching, with the shortfall
    /// quantified; the same model at 20 must pass.
    ///
    /// **Result** (2026-09-16): shortfall 12 and `is_satisfied() == Some(false)`
    /// at 4; `Some(true)` at 20.
    #[test]
    fn an_under_reaching_configuration_is_caught_and_quantified() {
        let (graph, radius) = unit_lattice();
        let under =
            physics_guided_lower_bound(PdeClass::Elliptic, &graph, radius, Some(4)).unwrap();
        assert_eq!(under.is_satisfied(), Some(false));
        assert_eq!(under.shortfall(), Some(12));

        let over =
            physics_guided_lower_bound(PdeClass::Elliptic, &graph, radius, Some(20)).unwrap();
        assert_eq!(over.is_satisfied(), Some(true));
        assert_eq!(over.shortfall(), Some(0));
    }

    /// **Methodology.** "Not checked" and "failed" must not be the same value.
    /// With no configured count, `is_satisfied` must be `None`.
    ///
    /// **Result** (2026-09-16): `None`, and `reach` is `None` too.
    #[test]
    fn an_unchecked_model_is_not_reported_as_failing() {
        let (graph, radius) = unit_lattice();
        let bound = physics_guided_lower_bound(PdeClass::Elliptic, &graph, radius, None).unwrap();
        assert_eq!(bound.is_satisfied(), None);
        assert_eq!(bound.reach, None);
    }

    /// **Methodology — refining the mesh makes the elliptic bound worse, which
    /// is the counter-intuitive consequence worth pinning.** Halving the mesh
    /// spacing doubles the diameter and therefore doubles the required
    /// iterations, even though the physics has not changed. That is why the
    /// paper's point is a warning and not merely a formula: an elliptic solver
    /// that works on a coarse mesh can fail on a finer one for purely
    /// architectural reasons.
    ///
    /// **Result** (2026-09-16): printed under `--nocapture`.
    #[test]
    fn refining_the_mesh_raises_the_elliptic_bound() {
        let mut row = Vec::new();
        for n in [5usize, 9, 17] {
            let spacing = 1.0 / (n as f64 - 1.0);
            let mut points = Vec::new();
            for i in 0..n {
                for j in 0..n {
                    points.push(vec![i as f64 * spacing, j as f64 * spacing]);
                }
            }
            let radius = spacing * 1.01;
            let graph = Graph::radius_graph(&points, radius).unwrap();
            let bound =
                physics_guided_lower_bound(PdeClass::Elliptic, &graph, radius, None).unwrap();
            row.push((n, bound.required));
        }
        println!("elliptic iteration bound against mesh resolution: {row:?}");
        assert!(row[0].1 < row[1].1 && row[1].1 < row[2].1);
    }

    /// **Methodology.** A disconnected mesh must be refused for the
    /// infinite-speed classes rather than given a finite bound — no iteration
    /// count makes an unreachable node reachable. A hyperbolic problem on the
    /// same mesh is still answerable, because its bound is metric rather than
    /// topological.
    ///
    /// **Result** (2026-09-16): elliptic errors, hyperbolic succeeds.
    #[test]
    fn a_disconnected_mesh_has_no_elliptic_bound() {
        let graph = Graph::from_undirected_edges(4, &[(0, 1), (2, 3)]).unwrap();
        assert!(physics_guided_lower_bound(PdeClass::Elliptic, &graph, 0.1, None).is_err());
        assert!(physics_guided_lower_bound(
            PdeClass::Hyperbolic {
                signal_speed: 1.0,
                time_step: 0.1
            },
            &graph,
            0.1,
            None
        )
        .is_ok());
    }

    /// **Methodology.** Malformed inputs must be refused: a non-positive hop
    /// length, signal speed or time step.
    ///
    /// **Result.** All three rejected (2026-09-16).
    #[test]
    fn malformed_bound_inputs_are_refused() {
        let (graph, _) = unit_lattice();
        assert!(physics_guided_lower_bound(PdeClass::Elliptic, &graph, 0.0, None).is_err());
        assert!(physics_guided_lower_bound(
            PdeClass::Hyperbolic {
                signal_speed: 0.0,
                time_step: 0.1
            },
            &graph,
            0.1,
            None
        )
        .is_err());
        assert!(physics_guided_lower_bound(
            PdeClass::Hyperbolic {
                signal_speed: 1.0,
                time_step: -0.1
            },
            &graph,
            0.1,
            None
        )
        .is_err());
    }
}
