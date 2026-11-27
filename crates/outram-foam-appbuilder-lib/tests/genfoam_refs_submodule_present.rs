// SPDX-License-Identifier: AGPL-3.0-only
//! The GeN-Foam differential-test references live in a git submodule.
//!
//! Since 2026-11-27 (maintainer decision) the `*.ref.txt.gz` files that the
//! `genfoam_*_vs_upstream` tests compare against are not tracked in this
//! repository. They are the `genfoam-vs-upstream-refs` branch of
//! `theodoreOnzGit/C-_YouTube_OpenFoam`, mounted as a submodule at
//! `verification_and_validation/genfoam_vs_upstream_gh1361/refs`.
//!
//! A plain clone leaves that path an empty directory, and every comparison
//! test then fails with a bare "No such file or directory". This test fails
//! first and says why, with the command that fixes it.

use std::path::Path;

/// The fetch command, printed when the submodule is missing.
const FETCH: &str = "git submodule update --init \
crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/refs";

#[test]
fn genfoam_reference_submodule_is_fetched() {
    let refs = Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs");
    let count = std::fs::read_dir(&refs)
        .map(|d| d.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().ends_with(".ref.txt.gz")).count())
        .unwrap_or(0);
    assert!(
        count > 0,
        "the GeN-Foam reference submodule at {} is not fetched (no *.ref.txt.gz files). Run:\n    {FETCH}",
        refs.display()
    );
}
