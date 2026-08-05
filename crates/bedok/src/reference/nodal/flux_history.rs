//! The rolling flux history the source iteration keeps for acceleration.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: the `scalar_flux` matrix of `sanodaldiffusion_solverxyz.m`, whose
//! comment reads "history of 5 for acceleration schemes, can increase in
//! needed", and its consumer `fiss_src_extrapolatexyz.m`.

/// A fixed-depth history of scalar-flux iterates, newest first.
///
/// Column 0 is the current iterate; column `j` is the iterate from `j` source
/// iterations ago. Every column is a full state vector of length `philenf`, in
/// neutrons cm⁻² s⁻¹ up to the arbitrary normalisation the eigenvalue problem
/// leaves free.
///
/// The default depth is 5, matching the MATLAB's `ones(philenf,5)`. Only the
/// first four columns are read (by the fission-source extrapolation); the
/// fifth is carried but unused, exactly as in the reference.
#[derive(Debug, Clone, PartialEq)]
pub struct FluxHistory {
    columns: Vec<Vec<f64>>,
}

impl FluxHistory {
    /// The MATLAB default history depth, 5.
    pub const DEFAULT_DEPTH: usize = 5;

    /// A history of `depth` identical columns, each `len` entries of `value` —
    /// MATLAB `value*ones(philenf, depth)`.
    ///
    /// # Panics
    ///
    /// If `depth` is zero.
    #[must_use]
    pub fn filled(len: usize, depth: usize, value: f64) -> Self {
        assert!(depth > 0, "history depth must be positive");
        Self {
            columns: vec![vec![value; len]; depth],
        }
    }

    /// A history whose every column is a copy of `flux` — the MATLAB's
    /// `repmat(initflux(:,1),1,nh)` warm-start path.
    #[must_use]
    pub fn broadcast(flux: &[f64], depth: usize) -> Self {
        assert!(depth > 0, "history depth must be positive");
        Self {
            columns: vec![flux.to_vec(); depth],
        }
    }

    /// A history built from explicit columns, newest first.
    ///
    /// # Panics
    ///
    /// If `columns` is empty or the columns have different lengths.
    #[must_use]
    pub fn from_columns(columns: Vec<Vec<f64>>) -> Self {
        assert!(!columns.is_empty(), "history needs at least one column");
        let len = columns[0].len();
        assert!(
            columns.iter().all(|c| c.len() == len),
            "history columns must all be the same length"
        );
        Self { columns }
    }

    /// Number of columns held.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.columns.len()
    }

    /// Length of each column, i.e. the state-vector length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns[0].len()
    }

    /// Whether the state vectors are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns[0].is_empty()
    }

    /// The current iterate — MATLAB `scalar_flux(:,1)`.
    #[must_use]
    pub fn current(&self) -> &[f64] {
        &self.columns[0]
    }

    /// Column `j`, counting back from the current iterate at `j = 0`.
    ///
    /// # Panics
    ///
    /// If `j >= depth()`.
    #[must_use]
    pub fn column(&self, j: usize) -> &[f64] {
        &self.columns[j]
    }

    /// Overwrites the current iterate without shifting the history.
    ///
    /// # Panics
    ///
    /// If the length does not match.
    pub fn set_current(&mut self, flux: Vec<f64>) {
        assert_eq!(flux.len(), self.len(), "flux length");
        self.columns[0] = flux;
    }

    /// Shifts every column back one place and installs `flux` as the current
    /// iterate, dropping the oldest — the MATLAB's
    /// `for j=size(...,2)-1:-1:1, scalar_flux(:,j+1)=scalar_flux(:,j); end`
    /// followed by `scalar_flux(:,1)=scalar_flux_l_plus`.
    ///
    /// # Panics
    ///
    /// If the length does not match.
    pub fn push(&mut self, flux: Vec<f64>) {
        assert_eq!(flux.len(), self.len(), "flux length");
        self.columns.rotate_right(1);
        self.columns[0] = flux;
    }

    /// Multiplies every column by `factor` — the final renormalisation
    /// `scalar_flux = scalar_flux*(init_norm/norm_factor)`.
    pub fn scale(&mut self, factor: f64) {
        for col in &mut self.columns {
            for v in col.iter_mut() {
                *v *= factor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_shifts_the_history_and_drops_the_oldest() {
        let mut h = FluxHistory::from_columns(vec![vec![1.0], vec![2.0], vec![3.0]]);
        h.push(vec![0.0]);
        assert_eq!(h.column(0), &[0.0]);
        assert_eq!(h.column(1), &[1.0]);
        assert_eq!(h.column(2), &[2.0]);
        assert_eq!(h.depth(), 3);
    }

    #[test]
    fn the_default_depth_matches_the_matlab() {
        let h = FluxHistory::filled(4, FluxHistory::DEFAULT_DEPTH, 1.0);
        assert_eq!(h.depth(), 5);
        assert_eq!(h.len(), 4);
        assert_eq!(h.current(), &[1.0; 4]);
    }

    #[test]
    fn scale_touches_every_column() {
        let mut h = FluxHistory::from_columns(vec![vec![1.0], vec![2.0]]);
        h.scale(3.0);
        assert_eq!(h.column(0), &[3.0]);
        assert_eq!(h.column(1), &[6.0]);
    }
}
