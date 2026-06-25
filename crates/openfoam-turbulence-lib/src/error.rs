use thiserror::Error;

#[derive(Debug, Error)]
pub enum TurbulenceError {
    #[error("field size mismatch: {0}")]
    FieldSizeMismatch(String),
    #[error("turbulence model not initialised")]
    NotInitialised,
    #[error("negative turbulent quantity: {field} = {value}")]
    NegativeField { field: &'static str, value: f64 },
}
