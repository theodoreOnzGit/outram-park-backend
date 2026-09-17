//! Lagrangian transmutation & fission — competing-rate depletion.
//!
//! The whole point of the Lagrangian Monte Carlo approach is that transmutation
//! and fission need **no burnup matrix**: each atom samples waiting times from
//! competing exponential clocks (decay, `(n,gamma)`, `(n,2n)`, fission), and the
//! population distribution emerges from the ensemble. There is no stiff Bateman
//! system, no CRAM, no matrix exponential.
//!
//! The implementation lives with the diffusion engine it is coupled to, in
//! [`crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion`]:
//! decay is wired to the crate's [`DecayLibrary`], and the neutron field enters
//! through the [`Transmutation`] input (currently a single explicit
//! `(n,gamma)` channel — the point where per-nuclide cross sections and fission
//! yields from `njoy-outram-park-fork` / `outram-mc-libs` flux maps plug in).
//!
//! The competing-rates framework is described in the crate `CLAUDE.md`
//! ("MC transmutation design sketch"): total rate `lambda = lambda_decay +
//! phi*sigma_ng + phi*sigma_n2n + phi*sigma_f`, waiting time `Exp(lambda)`, and
//! the event chosen by the usual competing-rates method. Fission-fragment yields
//! come from the ENDF/B-VIII.0 data available via `openmc-endf-8-depletion-lib-b`.
//!
//! [`DecayLibrary`]: crate::nuclide_reaction_and_decay_data::decay_library::DecayLibrary
//! [`Transmutation`]: crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::Transmutation

#[doc(inline)]
pub use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::{
    sample_event_time, DepletionOutcome, Transmutation,
};
