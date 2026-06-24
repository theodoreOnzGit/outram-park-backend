/// Errors produced by the specie-level thermophysics layer.
#[derive(Debug, Clone, PartialEq)]
pub enum ThermoError {
    /// Newton T-inversion exhausted all iterations without meeting the
    /// convergence tolerance (|ΔT/T| < 1e-6). Carries the last iterate.
    NonConvergent { max_iter: usize, last_t: f64 },
}

impl std::fmt::Display for ThermoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThermoError::NonConvergent { max_iter, last_t } => write!(
                f,
                "Newton T-inversion did not converge after {max_iter} iterations \
                 (last T = {last_t:.2} K)"
            ),
        }
    }
}

impl std::error::Error for ThermoError {}
