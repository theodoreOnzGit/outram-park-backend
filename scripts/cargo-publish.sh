#!/usr/bin/env bash
set -euo pipefail

# Publishes the dependency-graph pipeline from njoy-outram-park-fork through
# outram-park-digital-twin-engine, in the topological order derived from
# `cargo metadata --format-version 1 --no-deps` (see
# docs/workspace-maintenance.md "Publishing to crates.io" -- do not hand-edit
# this order without re-deriving it the same way).
#
# This script does NOT bump versions. Bump crates/<name>/Cargo.toml and the
# matching pin in the root [workspace.dependencies] by hand first (root
# CLAUDE.md: "Never auto-bump versions... only when explicitly requested"),
# commit, then run this.
#
# bedok and outram-park-fork-offbeat are deliberately NOT in this list: bedok
# has publish = false (unvalidated third-party MATLAB port, gated on purpose),
# and nothing in this pipeline depends on outram-park-fork-offbeat.

crates=(
    chem-eng-real-time-process-control-simulator
    outram-foam-basic-lib
    njoy-outram-park-fork
    outram-park-fork-coolprop
    outram-park-fork-dwsim-libs
    tuas_boussinesq_solver
    outram-mc-libs
    outram-foam-multiphase
    outram-foam-turbulence-lib
    teh-o-prke
    outram-foam-appbuilder-lib
    boon-lay
    tampines-steam-tables
    nee_soon
    tampines
    outram-park-digital-twin-engine
)

for crate in "${crates[@]}"; do
    echo "Publishing ${crate}..."
    cargo publish -p "${crate}"
    echo "Waiting for crates.io index to update..."
    sleep 30
done

echo "Done!"
