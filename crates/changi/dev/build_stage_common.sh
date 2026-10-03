# SPDX-License-Identifier: GPL-3.0
#
# Shared set-up for the per-stage FLEXPART reference builds
# (dev/build_reference_{interp,advance,stochastic}.sh). Sourced, not run.
#
# Provides CRATE_DIR, SRC, OUT, BUILD (a scratch directory, never inside the
# repository), FFLAGS, and nest_par_mod <dir>, which writes the nest-configured
# copy of par_mod.f90 into <dir> and checks the substitution happened.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$CRATE_DIR/upstream_source/FLEXPART/src"
OUT="$CRATE_DIR/tests/data"
BUILD="${FLEXPART_STAGE_BUILD:-$(mktemp -d)}"
# -mcmodel=medium: com_mod's static arrays exceed 2 GB at real(8).
FFLAGS=(-O2 -mcmodel=medium -std=legacy -fallow-argument-mismatch -w)
# The real(8) build. -fdefault-double-8 keeps `double precision` at 8 bytes;
# without it, -fdefault-real-8 promotes it to real(16).
REAL8=(-fdefault-real-8 -fdefault-double-8)

if [ ! -d "$SRC" ]; then
  echo "upstream FLEXPART clone not found at $SRC" >&2
  echo "  git clone https://github.com/flexpart/flexpart.git $CRATE_DIR/upstream_source/FLEXPART" >&2
  echo "  git -C $CRATE_DIR/upstream_source/FLEXPART checkout 3d7eebf" >&2
  exit 1
fi
command -v gfortran >/dev/null || { echo "gfortran not installed" >&2; exit 1; }
mkdir -p "$OUT" "$BUILD"

# par_mod.f90 is FLEXPART's user-edited configuration file. The shipped copy
# sets maxnests=0, nxmaxn=0, nymaxn=0, so nests cannot be driven; this writes
# a copy with ONLY that line changed. Every other upstream file is verbatim.
nest_par_mod () {
  local dir="$1"
  sed 's/maxnests=0,nxmaxn=0,nymaxn=0/maxnests=1,nxmaxn=12,nymaxn=12/' \
    "$SRC/par_mod.f90" > "$dir/par_mod.f90"
  [ "$(diff "$SRC/par_mod.f90" "$dir/par_mod.f90" | grep -c '^[<>]')" = 2 ] \
    || { echo "nest par_mod substitution failed" >&2; exit 1; }
}

INTERP_FILES=(
  "$SRC/interpol_mod.f90" "$SRC/hanna_mod.f90"
  "$SRC/interpol_all.f90" "$SRC/interpol_all_nests.f90"
  "$SRC/interpol_misslev.f90" "$SRC/interpol_misslev_nests.f90"
  "$SRC/interpol_wind.f90" "$SRC/interpol_wind_nests.f90"
  "$SRC/interpol_wind_short.f90" "$SRC/interpol_wind_short_nests.f90"
  "$SRC/interpol_vdep.f90" "$SRC/interpol_vdep_nests.f90"
)

STEP_FILES=(
  "$SRC/cmapf_mod.f90" "${INTERP_FILES[@]}"
  "$SRC/hanna.f90" "$SRC/hanna1.f90" "$SRC/hanna_short.f90" "$SRC/cbl.f90"
  "$SRC/re_initialize_particle.f90" "$SRC/initialize_cbl_vel.f90"
  "$SRC/initialize.f90" "$SRC/advance.f90" "$SRC/get_vdep_prob.f90"
  "$SRC/get_settling.f90" "$SRC/dynamic_viscosity.f90" "$SRC/windalign.f90"
)
