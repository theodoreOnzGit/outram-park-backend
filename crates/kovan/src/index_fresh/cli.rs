//! `kovan-cli index --fresh` (GitHub #780): the headless path to
//! [`super::run_fresh`], the same function the app's "Index fresh" button
//! calls. Reached by `tests/index_fresh_cli.rs`.

use std::path::{Path, PathBuf};

use crate::commands::index_control::RunControl;

use super::plan::{run_fresh, FreshChoices};
use super::root_file::{
    inspect_root, keystore_founders, last_good_committed_root, CorruptAction, RootState,
};

/// The folder `kovan-cli index` works on. An explicit `--workspace` that
/// is any Rust workspace or single crate ([`super::detect_project`]) is
/// taken as it is (#780: foreign repositories); otherwise the usual
/// outram-park discovery ([`crate::commands::workspace::resolve`]), so a
/// bare `kovan-cli index` behaves as before.
pub fn resolve_index_root(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(dir) = explicit {
        if super::detect_project(dir).is_ok() {
            return Ok(dir.to_path_buf());
        }
    }
    crate::commands::workspace::resolve(explicit)
        .map(|(root, _)| root)
        .map_err(|e| e.to_string())
}

/// The CLI's choices from its flags: the founder (else the one keystore
/// identity, else UNSET) and, for a corrupt root, `restore` (the newest
/// committed version that parses; an error when there is none) or `fresh`.
pub fn choices_from_flags(
    root: &Path,
    founder: Option<String>,
    corrupt_root: Option<&str>,
    scip: Option<PathBuf>,
) -> Result<FreshChoices, String> {
    let founder = founder.or_else(|| keystore_founders().default_founder());
    let corrupt = match (corrupt_root, inspect_root(root)) {
        (Some("restore"), RootState::Corrupt { .. }) => match last_good_committed_root(root)? {
            Some(c) => {
                eprintln!("index-fresh: restoring kovan_root.toml from {} ({})", c.commit, c.date);
                Some(CorruptAction::Restore { commit: c.commit })
            }
            None => return Err("--corrupt-root restore: no committed kovan_root.toml parses; use --corrupt-root fresh".into()),
        },
        (Some("fresh"), RootState::Corrupt { .. }) => Some(CorruptAction::StartFresh),
        (Some(other), RootState::Corrupt { .. }) => return Err(format!("--corrupt-root {other}: use restore or fresh")),
        _ => None,
    };
    Ok(FreshChoices {
        founder,
        corrupt,
        scip,
    })
}

/// `kovan-cli index --fresh` (module doc).
pub fn run_cli(
    root: &Path,
    founder: Option<String>,
    corrupt_root: Option<&str>,
    scip: Option<PathBuf>,
) -> Result<(), String> {
    let choices = choices_from_flags(root, founder, corrupt_root, scip)?;
    run_fresh(root, &choices, &RunControl::default())
        .map(|_| ())
        .map_err(|e| e.to_string())
}
