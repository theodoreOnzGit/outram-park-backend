#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# gh:#1448: extract NJOY2016.80 reconr.f90 csunr1 (l.3826-4077), csunr2 (:4079-4317),
# unfac (:4473-4496) and gnrl (:4498-4644) VERBATIM, after checking the SHA-256 of the file.
# The routines use module variables of `reconr` (res, ibaset, isect, ilast, enxt, elst, snxt,
# slst); driver.f90 declares them in a module that `contains` the extracted text.
# Usage: extract.sh outfile   (NJOY_UPSTREAM_SOURCE = folder holding NJOY2016/)
# NJOY2016 is modified-BSD-3 (LANL); see crates/njoy-outram-park-fork/LICENSE.njoy.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
up=${NJOY_UPSTREAM_SOURCE:-$here/../../../upstream_source}
src=$up/NJOY2016/src/reconr.f90
echo "054ede7a59e1c39cf3e72105d8a0b95a0fb1d8df0882eca6b949e765b62bf5db  $src" | sha256sum -c -
{
  echo "! EXTRACTED by extract.sh from NJOY2016.80 reconr.f90 l.3826-4317 and l.4473-4644"
  sed -n '3826,4317p' "$src"
  sed -n '4473,4644p' "$src"
} > "$1"
