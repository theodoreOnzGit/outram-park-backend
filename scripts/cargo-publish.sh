#!/usr/bin/env bash
set -euo pipefail


echo "Publishing tuas_boussinesq_solver..."
cargo publish -p tuas_boussinesq_solver

echo "Waiting for crates.io index to update..."
sleep 30

echo "Publishing tampines-steam-tables..."
cargo publish -p tampines-steam-tables


echo "Publishing teh-o-prke"
cargo publish --workspace -p teh-o-prke

echo "Done!"
