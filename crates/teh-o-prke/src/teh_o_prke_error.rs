use thiserror::Error;

/// Master Error type of this crate
#[derive(Debug, Error)]
pub enum TehOPrkeError {
    /// matrix solve error (e.g. singular coefficient matrix)
    #[error("matrix solve error: {0}")]
    ShapeMismatch(String),

    /// it's a generic error which is a placeholder since I used
    /// so many string errors
    #[error("Placeholder Error Type for Strings{0} ")]
    GenericStringError(String),

    /// [`crate::nordheim_fuchs`]'s exact timestepper requires a strictly
    /// negative fuel feedback coefficient (alpha_f < 0, self-limiting
    /// negative feedback) for its closed-form solution to stay
    /// real-valued; a non-negative value describes a non-self-limiting
    /// excursion this model does not support.
    #[error("Nordheim-Fuchs requires fuel feedback coefficient alpha_f < 0 K^-1 (self-limiting negative feedback); got {0} K^-1")]
    NonNegativeFuelFeedbackCoefficient(f64),

    /// [`crate::nordheim_fuchs`]'s prompt neutron generation time Lambda
    /// must be strictly positive.
    #[error("prompt neutron generation time Lambda must be > 0 s; got {0} s")]
    NonPositivePromptNeutronGenerationTime(f64),

    /// [`crate::nordheim_fuchs`]'s lumped fuel heat capacity C_f must be
    /// strictly positive.
    #[error("fuel heat capacity C_f must be > 0 J/K; got {0} J/K")]
    NonPositiveFuelHeatCapacity(f64),

    /// [`crate::delayed_neutron_layer`]'s per-group decay constant
    /// `lambda_i` must be strictly positive (each precursor group is a
    /// stable first-order lag of time constant `tau_i = 1/lambda_i`); a
    /// non-positive `lambda_i` has no finite time constant.
    #[error("delayed-neutron decay constant lambda_i must be > 0 s^-1; got {0} s^-1")]
    NonPositiveDelayedDecayConstant(f64),

    /// [`crate::feedback_mechanisms::fast_reactor::RodWorthMap`]: a
    /// driveline expansion outside the closed range of the rod-worth map,
    /// under the default
    /// [`crate::feedback_mechanisms::fast_reactor::RodWorthOutOfRange::Error`]
    /// policy (gh:#1326, maintainer decision 2026-11-10). Holds the
    /// expansion and the map's lower and upper ends, in metres.
    #[error("driveline expansion {expansion_m} m is outside the rod-worth map's range [{min_m}, {max_m}] m")]
    ExpansionOutsideRodWorthMap {
        /// The expansion asked for \[m\].
        expansion_m: f64,
        /// The map's smallest expansion \[m\].
        min_m: f64,
        /// The map's largest expansion \[m\].
        max_m: f64,
    },
}

///  converts ThermalHydraulicsLibError from string error
impl From<String> for TehOPrkeError {
    fn from(value: String) -> Self {
        Self::GenericStringError(value)
    }
}

impl Into<String> for TehOPrkeError {
    fn into(self) -> String {
        match self {
            TehOPrkeError::ShapeMismatch(s) => s,
            TehOPrkeError::GenericStringError(s) => s,
            TehOPrkeError::NonNegativeFuelFeedbackCoefficient(_)
            | TehOPrkeError::NonPositivePromptNeutronGenerationTime(_)
            | TehOPrkeError::NonPositiveFuelHeatCapacity(_)
            | TehOPrkeError::NonPositiveDelayedDecayConstant(_)
            | TehOPrkeError::ExpansionOutsideRodWorthMap { .. } => self.to_string(),
        }
    }
}
