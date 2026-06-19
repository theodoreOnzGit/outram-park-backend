use thiserror::Error;

/// Master Error type of this crate
#[derive(Debug, Error)]
pub enum TehOPrkeError {
    /// linear algebra error
    #[error("linear algebra error")]
    LinalgError(#[from] ndarray_linalg::error::LinalgError),

    /// empty mass flowrate vector error 
    ///
    /// this case is where the mass flowrate vector in a control 
    /// volume is empty, 
    /// so we can't calculate a courant number

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
            TehOPrkeError::LinalgError(_) => {
                self.to_string()
            },
            TehOPrkeError::GenericStringError(string) => {
                string
            },

        }
    }
}

