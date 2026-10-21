//! Workspace or single crate, from `Cargo.toml` alone (maintainer,
//! 2026-10-07): a `[workspace]` table makes it a workspace (also when it
//! has a `[package]` as well, a root-package workspace); otherwise a
//! `[package]` table makes it a single crate. No other heuristic, and the
//! user is not asked.

use std::path::{Path, PathBuf};

/// What a folder's `Cargo.toml` declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    /// `[workspace]`: every member gets its index.
    Workspace,
    /// `[package]` and no `[workspace]`: one crate's worth of index.
    SingleCrate,
}

impl ProjectKind {
    /// For display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Cargo workspace",
            Self::SingleCrate => "single crate",
        }
    }
}

/// Why a folder is not a Rust project kovan can index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// No `Cargo.toml` in the folder.
    NoManifest(PathBuf),
    /// `Cargo.toml` could not be read or is not TOML.
    Unreadable { path: PathBuf, error: String },
    /// `Cargo.toml` has neither `[workspace]` nor `[package]`.
    NeitherWorkspaceNorPackage(PathBuf),
}

impl std::fmt::Display for DetectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoManifest(d) => write!(
                f,
                "{} has no Cargo.toml: choose a Rust workspace or crate folder",
                d.display()
            ),
            Self::Unreadable { path, error } => write!(f, "{}: {error}", path.display()),
            Self::NeitherWorkspaceNorPackage(p) => {
                write!(
                    f,
                    "{} declares neither [workspace] nor [package]",
                    p.display()
                )
            }
        }
    }
}

impl std::error::Error for DetectError {}

/// Workspace or single crate (module doc).
pub fn detect_project(dir: &Path) -> Result<ProjectKind, DetectError> {
    let path = dir.join("Cargo.toml");
    if !path.is_file() {
        return Err(DetectError::NoManifest(dir.to_path_buf()));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| DetectError::Unreadable {
        path: path.clone(),
        error: e.to_string(),
    })?;
    let table: toml::Table = toml::from_str(&text).map_err(|e| DetectError::Unreadable {
        path: path.clone(),
        error: e.to_string(),
    })?;
    if table.get("workspace").is_some_and(toml::Value::is_table) {
        Ok(ProjectKind::Workspace)
    } else if table.get("package").is_some_and(toml::Value::is_table) {
        Ok(ProjectKind::SingleCrate)
    } else {
        Err(DetectError::NeitherWorkspaceNorPackage(path))
    }
}
