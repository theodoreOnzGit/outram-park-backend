//! Stand-alone depletion driver — assemble the burnup matrix and deplete.
//!
//! This is the precomputed-input (stand-alone) mode of ONIX: the caller
//! supplies decay data, one-group reaction rates, fission yields, and an
//! initial inventory; the driver assembles the burnup matrix `A` and advances
//! the inventory over one or more timesteps with [`crate::cram::cram16`]. There
//! is **no** neutron-transport / OpenMC coupling here — reaction rates are
//! taken as given (see the crate-level scope notes).
//!
//! ## Provenance (GPLv3 relicensing of MIT upstream)
//!
//! Driver flow mirrors ONIX (open-source, MIT; commit `7328dc6`):
//!   * matrix assembly — `onix/salameche/mat_builder.py` (`get_xs_mat`,
//!     `get_decay_mat`, `get_initial_vect`),
//!   * `A = B·1e-24·φ + C`, `At = A·Δt`, `CRAM16(At, N)` — `onix/salameche/
//!     burn.py:187–194` (`burn_microstep`),
//!   * multi-step loop — `onix/salameche/burn.py:69` (`burn_cell`) over
//!     macrosteps.
//!
//! Independent Rust re-implementation; OUTRAM PARK fork relicenses under
//! **GPL-3.0-only** (MIT is GPL-3.0-compatible).

use std::collections::HashMap;

use crate::chain::{DecayData, FissionYields, ReactionRates};
use crate::cram::{cram16, CramError};
use crate::matrix::BurnupMatrix;
use crate::nuclide::{Nuclide, ZamId};

/// A nuclide's index in the depletion vector (its row/column in the matrix).
pub type NuclideIndex = usize;

/// Errors from building or running a [`DepletionSystem`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DepletionError {
    /// The same nuclide was registered twice.
    #[error("nuclide {zamid} registered more than once")]
    DuplicateNuclide {
        /// The offending packed id.
        zamid: ZamId,
    },
    /// A supplied initial-inventory map referenced a nuclide not in the system.
    #[error("initial inventory references nuclide {zamid} that is not in the system")]
    UnknownNuclide {
        /// The offending packed id.
        zamid: ZamId,
    },
    /// The CRAM solve failed.
    #[error("CRAM solve failed: {0}")]
    Cram(#[from] CramError),
}

/// A stand-alone depletion system: a fixed set of nuclides plus their decay
/// data, reaction rates, and fission yields.
///
/// The nuclide set fixes the matrix ordering: nuclide registered `k`-th occupies
/// row/column `k`. Number densities are carried in whatever unit the caller uses
/// for the initial inventory (atoms, or atoms·cm⁻³); the driver is unit-agnostic
/// on the inventory as long as it is consistent.
///
/// Reaction rates are held **separately from decay data** so they can be
/// replaced between burnup steps (changing flux/spectrum) while the decay data
/// stays fixed — see [`DepletionSystem::set_reaction_rates`] and
/// [`DepletionSystem::deplete_multi`].
#[derive(Debug, Clone)]
pub struct DepletionSystem {
    nuclides: Vec<Nuclide>,
    index: HashMap<ZamId, NuclideIndex>,
    decay: Vec<DecayData>,
    rates: Vec<ReactionRates>,
    fission_yields: Vec<FissionYields>,
}

impl Default for DepletionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DepletionSystem {
    /// An empty system with no nuclides.
    pub fn new() -> Self {
        Self {
            nuclides: Vec::new(),
            index: HashMap::new(),
            decay: Vec::new(),
            rates: Vec::new(),
            fission_yields: Vec::new(),
        }
    }

    /// Register a nuclide with its decay data, reaction rates, and fission
    /// yields. Returns the assigned [`NuclideIndex`].
    ///
    /// Order of registration is the order of rows/columns in the assembled
    /// matrix. Registering the same nuclide (same [`Nuclide::zamid`]) twice is
    /// an error.
    pub fn add_nuclide(
        &mut self,
        nuclide: Nuclide,
        decay: DecayData,
        rates: ReactionRates,
        fission_yields: FissionYields,
    ) -> Result<NuclideIndex, DepletionError> {
        let zamid = nuclide.zamid();
        if self.index.contains_key(&zamid) {
            return Err(DepletionError::DuplicateNuclide { zamid });
        }
        let idx = self.nuclides.len();
        self.nuclides.push(nuclide);
        self.index.insert(zamid, idx);
        self.decay.push(decay);
        self.rates.push(rates);
        self.fission_yields.push(fission_yields);
        Ok(idx)
    }

    /// Number of nuclides in the system (matrix dimension).
    pub fn len(&self) -> usize {
        self.nuclides.len()
    }

    /// Whether the system holds no nuclides.
    pub fn is_empty(&self) -> bool {
        self.nuclides.is_empty()
    }

    /// The nuclides in matrix order.
    pub fn nuclides(&self) -> &[Nuclide] {
        &self.nuclides
    }

    /// The index of a nuclide, or `None` if it is not tracked.
    pub fn index_of(&self, nuclide: Nuclide) -> Option<NuclideIndex> {
        self.index.get(&nuclide.zamid()).copied()
    }

    /// Replace the reaction rates of one nuclide (for a new burnup step).
    ///
    /// `nuclide` must already be registered. Decay data and fission yields are
    /// untouched. This is how a multi-step burnup with a changing flux is
    /// modelled — update rates, then re-assemble and re-deplete.
    pub fn set_reaction_rates(
        &mut self,
        nuclide: Nuclide,
        rates: ReactionRates,
    ) -> Result<(), DepletionError> {
        let zamid = nuclide.zamid();
        let idx = self
            .index
            .get(&zamid)
            .copied()
            .ok_or(DepletionError::UnknownNuclide { zamid })?;
        self.rates[idx] = rates;
        Ok(())
    }

    /// Assemble the burnup matrix `A` (units `1/s`) from the current data.
    ///
    /// Mirrors ONIX `get_decay_mat` + `get_xs_mat` combined as
    /// `B·1e-24·φ + C` (`burn.py:187`), except reaction rates are already in
    /// `1/s`:
    ///
    /// * diagonal `A[i][i] = -(λ_i + Σ_c r_{i,c})` — total decay + total removal,
    /// * decay off-diagonal: for each parent `j`, mode with branching `b`,
    ///   `A[daughter][j] += λ_j · b`,
    /// * reaction off-diagonal: for each parent `j`, non-fission channel with
    ///   rate `r`, `A[daughter][j] += r`,
    /// * fission off-diagonal: for each fissile parent `j` with fission rate
    ///   `r_fis` and product `p` with yield `y`, `A[p][j] += r_fis · y`.
    ///
    /// Gains to daughters not tracked in the system are silently dropped (they
    /// leave the modelled chain), exactly as ONIX skips parents/products absent
    /// from the `index_dict`.
    pub fn build_matrix(&self) -> BurnupMatrix {
        let n = self.nuclides.len();
        let mut a = BurnupMatrix::zeros(n);

        for j in 0..n {
            let parent = self.nuclides[j];

            // --- Decay ---
            let dd = &self.decay[j];
            // Diagonal loss: total decay constant.
            a.add(j, j, -dd.lambda_total);
            // Off-diagonal gains to tracked daughters.
            for &(mode, branch) in &dd.branches {
                if let Some(daughter) = mode.daughter(parent) {
                    if let Some(&i) = self.index.get(&daughter.zamid()) {
                        a.add(i, j, dd.lambda_total * branch);
                    }
                }
            }

            // --- Neutron reactions ---
            let rr = &self.rates[j];
            // Diagonal loss: total removal (all channels incl. fission).
            a.add(j, j, -rr.total_removal());
            // Off-diagonal gains.
            for &(channel, rate) in &rr.channels {
                if channel.is_fission() {
                    // Fission: distribute over the yield table.
                    for &(product, y) in &self.fission_yields[j].products {
                        if let Some(&i) = self.index.get(&product.zamid()) {
                            a.add(i, j, rate * y);
                        }
                    }
                } else if let Some(daughter) = channel.daughter(parent) {
                    if let Some(&i) = self.index.get(&daughter.zamid()) {
                        a.add(i, j, rate);
                    }
                }
            }
        }

        a
    }

    /// Build an initial-inventory vector from a `(nuclide, density)` map.
    ///
    /// Nuclides absent from `densities` start at `0.0`. Unknown nuclides (not
    /// registered) are an error. Units of `density` are the caller's inventory
    /// unit (atoms, or atoms·cm⁻³) and carry through unchanged.
    pub fn inventory_vector(
        &self,
        densities: &[(Nuclide, f64)],
    ) -> Result<Vec<f64>, DepletionError> {
        let mut v = vec![0.0; self.nuclides.len()];
        for &(nuc, dens) in densities {
            let zamid = nuc.zamid();
            let idx = self
                .index
                .get(&zamid)
                .copied()
                .ok_or(DepletionError::UnknownNuclide { zamid })?;
            v[idx] = dens;
        }
        Ok(v)
    }

    /// Deplete `n0` over a single timestep `dt` (seconds) via order-16 CRAM.
    ///
    /// Returns the depleted inventory `n(Δt) = exp(A·Δt)·n0` in the same units
    /// as `n0`. `n0.len()` must equal [`DepletionSystem::len`]. Negative CRAM
    /// artefacts are **not** clamped; call [`crate::cram::clamp_nonnegative`]
    /// on the result if the physicality filter is wanted.
    pub fn deplete(&self, n0: &[f64], dt: f64) -> Result<Vec<f64>, DepletionError> {
        let a = self.build_matrix();
        Ok(cram16(&a, dt, n0)?)
    }

    /// Multi-step depletion with a fixed matrix over each `dt` in `steps`.
    ///
    /// Chains single steps: the output of one step is the input to the next.
    /// The burnup matrix is **re-assembled at each step** from the current
    /// reaction rates, so to model a changing flux, call
    /// [`DepletionSystem::set_reaction_rates`] between the individual
    /// [`DepletionSystem::deplete`] calls instead. This convenience method
    /// keeps rates constant across all `steps`; each `dt` is in **seconds**.
    ///
    /// Returns the inventory after the last step (same units as `n0`).
    pub fn deplete_multi(&self, n0: &[f64], steps: &[f64]) -> Result<Vec<f64>, DepletionError> {
        let a = self.build_matrix();
        let mut n = n0.to_vec();
        for &dt in steps {
            n = cram16(&a, dt, &n)?;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactions::{DecayMode, ReactionChannel};

    #[test]
    fn build_matrix_two_step_decay_chain() {
        // A(Z=50,A=100) --beta-> B(Z=51,A=100) --beta-> C(Z=52,A=100), C stable.
        let a_nuc = Nuclide::new(50, 100, 0);
        let b_nuc = Nuclide::new(51, 100, 0);
        let c_nuc = Nuclide::new(52, 100, 0);
        let mut sys = DepletionSystem::new();
        sys.add_nuclide(
            a_nuc,
            DecayData::single_mode(2.0, DecayMode::BetaMinus),
            ReactionRates::none(),
            FissionYields::empty(),
        )
        .unwrap();
        sys.add_nuclide(
            b_nuc,
            DecayData::single_mode(0.5, DecayMode::BetaMinus),
            ReactionRates::none(),
            FissionYields::empty(),
        )
        .unwrap();
        sys.add_nuclide(c_nuc, DecayData::stable(), ReactionRates::none(), FissionYields::empty())
            .unwrap();

        let m = sys.build_matrix();
        // Diagonal losses.
        assert!((m.get(0, 0) - -2.0).abs() < 1e-15);
        assert!((m.get(1, 1) - -0.5).abs() < 1e-15);
        assert!((m.get(2, 2) - 0.0).abs() < 1e-15);
        // A feeds B; B feeds C.
        assert!((m.get(1, 0) - 2.0).abs() < 1e-15);
        assert!((m.get(2, 1) - 0.5).abs() < 1e-15);
        // Every column sums to zero: atoms are conserved (all daughters tracked).
        for cs in m.column_sums() {
            assert!(cs.abs() < 1e-14);
        }
    }

    #[test]
    fn capture_off_diagonal_uses_rate() {
        // A --(n,gamma)--> B, rate 1e-8 /s.
        let a_nuc = Nuclide::new(50, 100, 0);
        let b_nuc = Nuclide::new(50, 101, 0);
        let mut sys = DepletionSystem::new();
        sys.add_nuclide(
            a_nuc,
            DecayData::stable(),
            ReactionRates {
                channels: vec![(ReactionChannel::NGamma, 1e-8)],
            },
            FissionYields::empty(),
        )
        .unwrap();
        sys.add_nuclide(b_nuc, DecayData::stable(), ReactionRates::none(), FissionYields::empty())
            .unwrap();
        let m = sys.build_matrix();
        assert!((m.get(0, 0) - -1e-8).abs() < 1e-24);
        assert!((m.get(1, 0) - 1e-8).abs() < 1e-24);
    }
}
