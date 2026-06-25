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
        }
    }
}

