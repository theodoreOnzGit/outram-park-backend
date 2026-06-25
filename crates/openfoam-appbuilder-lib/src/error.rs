use thiserror::Error;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum AppBuilderError {
    #[error("I/O error reading {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("parse error in {file} at line {line}: {msg}")]
    Parse { file: String, line: usize, msg: String },
    #[error("missing required key '{key}' in {dict}")]
    MissingKey { key: &'static str, dict: &'static str },
    #[error("solver diverged after {iter} iterations (residual {residual:.3e})")]
    Diverged { iter: usize, residual: f64 },
    #[error("time limit reached: t = {t:.6} s")]
    TimeLimitReached { t: f64 },
}
