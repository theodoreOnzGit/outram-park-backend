//! **Recent code workspaces**: the Rust workspaces and crates the code map
//! was pointed at, most recent first (GitHub #780).
//!
//! Kept in its own file, `recent_code_workspaces.toml` in kovan's config
//! folder, apart from the Home screen's `recent_roots.toml` (kovan
//! libraries): a foreign workspace never enters the library list, and
//! nothing about it is written into outram-park's tree.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many workspaces the list keeps.
pub const CAP: usize = 10;

/// The list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaces {
    #[serde(default)]
    pub paths: Vec<PathBuf>,
}

impl RecentWorkspaces {
    /// Move `path` to the front (no duplicates), keeping at most [`CAP`].
    pub fn push(&mut self, path: &Path) {
        self.paths.retain(|p| p != path);
        self.paths.insert(0, path.to_path_buf());
        self.paths.truncate(CAP);
    }

    /// Read the list at `file`; empty when missing or unreadable (a lost
    /// recents list costs only convenience).
    pub fn load_from(file: &Path) -> RecentWorkspaces {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write the list to `file` (tmp then rename).
    pub fn save_to(&self, file: &Path) -> std::io::Result<()> {
        if let Some(d) = file.parent() {
            std::fs::create_dir_all(d)?;
        }
        let body = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        let tmp = file.with_extension("toml.tmp");
        std::fs::write(
            &tmp,
            format!("# kovan: recent code workspaces. Local, safe to delete.\n{body}"),
        )?;
        std::fs::rename(tmp, file)
    }
}

/// `<kovan config dir>/recent_code_workspaces.toml`, the config folder the
/// rest of kovan uses (`app/home.rs`, `app/setup.rs`). `None` under
/// `cargo test`, so tests never touch the user's real list.
pub fn default_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")
        .map(|d| d.config_dir().join("recent_code_workspaces.toml"))
}
