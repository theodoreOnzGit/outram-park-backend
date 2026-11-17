#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use thiserror::Error;

/// Master Error type of this crate
#[derive(Debug, Error)]
pub enum ChemEngProcessControlSimulatorError {
    /// it's a generic error which is a placeholder since I used
    /// so many string errors
    #[error("Placeholder Error Type for Strings{0} ")]
    GenericStringError(String),

    /// when transfer function is unstable when it should be
    /// stable
    #[error("Unstable Damping Factor for Stable Transfer Function")]
    UnstableDampingFactorForStableTransferFunction,

    #[error("wrong transfer function type")]
    WrongTransferFnType,

    #[cfg(feature = "std")]
    #[error("csv error")]
    CsvError(csv::Error),
}

#[cfg(feature = "std")]
impl From<csv::Error> for ChemEngProcessControlSimulatorError {
    fn from(csv_error: csv::Error) -> Self {
        Self::CsvError(csv_error)
    }
}

///  converts ThermalHydraulicsLibError from string error
impl From<String> for ChemEngProcessControlSimulatorError {
    fn from(value: String) -> Self {
        Self::GenericStringError(value)
    }
}

impl Into<String> for ChemEngProcessControlSimulatorError {
    fn into(self) -> String {
        match self {
            ChemEngProcessControlSimulatorError::GenericStringError(string) => string,
            ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction => {
                // just recursively calling, probably need to check
                // thermal hydraulics
                "unstable damping factor".to_owned()
            }
            ChemEngProcessControlSimulatorError::WrongTransferFnType => {
                "wrong transfer function type".to_owned()
            }
            #[cfg(feature = "std")]
            ChemEngProcessControlSimulatorError::CsvError(err) => err.to_string(),
        }
    }
}
