# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
#
# gh:#1790: make the stage-counting copy of TetGen 1.6.0 `predicates.cxx`.
# Every `return ...;` statement inside orient3d, orient3dadapt, insphere,
# insphereadapt, orient4d and orient4dadapt becomes
#   { TG_HIT(<fn>, <k>); return ...; }
# where <k> numbers that function's return statements in source order. Nothing
# else changes; `run.sh` checks that the counted build writes the same vector
# file, byte for byte, as the build from the untouched upstream file.
# Commented-out returns (`//if (...) return det;`) are skipped.
# A return spanning several lines gets its block closed after its first `;`.
# The CGAL `orient3d` / `insphere` are `#ifdef`'d out; their counters, if any,
# are never compiled.

/^REAL orient3d\(/      { fn = "FN_ORIENT3D"; k = 0 }
/^REAL orient3dadapt\(/ { fn = "FN_ORIENT3DADAPT"; k = 0 }
/^REAL insphere\(/      { fn = "FN_INSPHERE"; k = 0 }
/^REAL insphereadapt\(/ { fn = "FN_INSPHEREADAPT"; k = 0 }
/^REAL orient4d\(/      { fn = "FN_ORIENT4D"; k = 0 }
/^REAL orient4dadapt\(/ { fn = "FN_ORIENT4DADAPT"; k = 0 }
/^}/                    { if (fn != "") { print; fn = ""; next } }
{
  if (pending && index($0, ";")) {
    i = index($0, ";")
    $0 = substr($0, 1, i) " }" substr($0, i + 1)
    pending = 0
  } else if (fn != "" && $0 !~ /^[ \t]*\/\// && match($0, /return [^;]*;/)) {
    stmt = substr($0, RSTART, RLENGTH)
    $0 = substr($0, 1, RSTART - 1) "{ TG_HIT(" fn ", " k "); " stmt " }" substr($0, RSTART + RLENGTH)
    k++
  } else if (fn != "" && $0 !~ /^[ \t]*\/\// && match($0, /return /)) {
    # A return that continues on the next lines (`return orient4dadapt(pa,
    # ...);`): open the block here, close it after the first `;`.
    $0 = substr($0, 1, RSTART - 1) "{ TG_HIT(" fn ", " k "); " substr($0, RSTART)
    k++
    pending = 1
  }
  print
}
