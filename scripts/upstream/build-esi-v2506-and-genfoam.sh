#!/bin/bash
# Rebuild the upstream reference builds used by the upstream-parity work
# (epic gh:#1336): OpenFOAM ESI v2506 (a partial, dummy-Pstream build of exactly
# the subset the V&V records used) and GeN-Foam 652b3da0cc55 on top of it, into
# /home/user/upstream-build, with the SAME paths as before the container reset:
#
#   /home/user/upstream-build/OpenFOAM-v2506     ESI source tree + platforms/ + build/
#   /home/user/upstream-build/GeN-Foam-src       GeN-Foam at the pinned commit
#   /home/user/upstream-build/scripts/with-of.sh environment wrapper (also of-env.sh)
#   /root/OpenFOAM/user-v2506                    FOAM_USER_* (GeN-Foam libs and binary)
#   /home/user/upstream-build/gh1363/bin         real sonicFoam, rhoPimpleFoam, ...
#
# Research, education and V&V only (RESPONSIBLE_USE.md). Uses only open-source
# upstream sources (OpenFOAM is GPL-3.0, GeN-Foam is GPL-3.0).
#
# Usage:
#   scripts/upstream/build-esi-v2506-and-genfoam.sh [stage ...]
#     (no argument = every stage, in order; finished stages are skipped)
#   stages: prereqs fetch-esi env core prereq-libs closure meshing solvers
#           fetch-genfoam genfoam fetch-cfmesh cfmesh verify
#           (fetch-cfmesh and cfmesh: DAFoam cfMesh e944f6ba695f, gh:#1385; cfMesh
#           was built against ESI v2506, not OpenFOAM-dev, see the V&V README)
#   status          print which stages and libraries are done, disk, timings
#   env-only        (re)write scripts/with-of.sh and scripts/of-env.sh and stop
#   FORCE=1 stage   redo the named stage(s): ignores every stamp inside them
#
# Environment knobs (all optional):
#   WM_NCOMPPROCS     parallel compile jobs, default 2 (the box is shared)
#   KEEP_OBJECTS=1    keep the object files after each library links (default:
#                     delete them, as the earlier build did, to save disk)
#   MIN_FREE_GB       abort a stage when free space on / drops below this, default 5
#   UPSTREAM_BUILD_ROOT  override the root (the records use the default; do not
#                     change it unless you also change every record path)
#
# The script re-runs itself under `nice -n 10`. Every stage is idempotent and
# resumable: a stamp under $ROOT/.stamps marks each finished library and stage,
# and an interrupted wmake continues from its own dependency files.
# Logs: $ROOT/logs/<stage>.log ; timings: $ROOT/logs/timings.tsv

set -uo pipefail

if [ "$(nice)" -lt 10 ]; then exec nice -n 10 "$0" "$@"; fi

ROOT=${UPSTREAM_BUILD_ROOT:-/home/user/upstream-build}
N=${WM_NCOMPPROCS:-2}
KEEP_OBJECTS=${KEEP_OBJECTS:-0}
MIN_FREE_GB=${MIN_FREE_GB:-5}

OF=$ROOT/OpenFOAM-v2506
GF=$ROOT/GeN-Foam-src
DL=$ROOT/dl
STAMPS=$ROOT/.stamps
LOGS=$ROOT/logs
SCRIPTS=$ROOT/scripts
SCRATCH=$ROOT/gh1363/closure/scratch
SOLVBIN=$ROOT/gh1363/bin

# --- pinned inputs ----------------------------------------------------------
ESI_TGZ=OpenFOAM-v2506.tgz
# sha256 of the tarball fetched from SourceForge on 2026-11-12 (api=2506 patch=0).
# Trust-on-first-use: the maintainer's earlier tree is gone, so this is the value
# of the copy this script was written against. A mismatch is a loud warning, and
# the extracted tree must still report api=2506 patch=0 or the stage fails.
ESI_SHA256=63d26f48ae7ee9a7806a0ceb339ef8a0ba485a4714d54fbfb31e78e1a4849965
# Mirrors in order. dl.openfoam.com and develop.openfoam.com are the official
# hosts; the agent proxy denied both on 2026-11-12 (HTTP 403 on CONNECT), and
# SourceForge (the official project's file mirror) worked.
ESI_URLS=(
  "https://dl.openfoam.com/source/v2506/OpenFOAM-v2506.tgz"
  "https://sourceforge.net/projects/openfoam/files/v2506/OpenFOAM-v2506.tgz/download"
)
GF_URL=https://gitlab.com/foam-for-nuclear/GeN-Foam.git
GF_SHA=652b3da0cc558543b89e880812d910bf122b356c   # 652b3da0cc55, "docs: Add archive banner"
# cfMesh, DAFoam line (gh:#1385). The clone lives in the crate's gitignored
# upstream_source/ (override with CF_SRC), next to this repository's own copy of
# the three-part v2506 API patch.
CF_URL=https://github.com/DAFoam/cfmesh.git
CF_SHA=e944f6ba695fcefce73ad306074f6b3c0ea432ae   # e944f6ba695f
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
CFCRATE=$REPO/crates/outram-park-fork-cfmesh
CF=${CF_SRC:-$CFCRATE/upstream_source/cfmesh_dafoam}
CF_PATCH=$CFCRATE/verification_and_validation/upstream_cfmesh_gh1385/patches/cfmesh-e944f6ba695f-openfoam-v2506.patch

# --- helpers ----------------------------------------------------------------
log() { echo "[$(date +%H:%M:%S)] $*"; }
mkdir -p "$STAMPS" "$LOGS" "$SCRIPTS" "$DL" "$SOLVBIN" "$SCRATCH"

free_gb() { df --output=avail -BG / | tail -1 | tr -dc '0-9'; }
check_disk() {
  local f; f=$(free_gb)
  if [ "$f" -lt "$MIN_FREE_GB" ]; then
    log "ABORT: only ${f} GB free on / (floor ${MIN_FREE_GB} GB). Free space, then re-run."
    exit 2
  fi
}

retry() { # retry <tries> cmd...   (exponential backoff 5,10,20,... s)
  local tries=$1 i=1 d=5; shift
  until "$@"; do
    [ "$i" -ge "$tries" ] && return 1
    log "retry $i/$tries in ${d}s: $*"; sleep "$d"; d=$((d * 2)); i=$((i + 1))
  done
}

# run_stage <name> <fn> [args]: run fn in a subshell with set -e, logging to
# $LOGS/<name>.log, and write a stamp. Skips when the stamp exists.
run_stage() {
  local name=$1; shift
  if [ -f "$STAMPS/$name" ] && [ "${FORCE:-0}" != 1 ]; then
    log "skip  $name (stamp $(cat "$STAMPS/$name"))"; return 0
  fi
  check_disk
  local t0 t1; t0=$(date +%s)
  log "START $name"
  if ( set -e; "$@" ) >>"$LOGS/$name.log" 2>&1; then
    t1=$(date +%s)
    echo "$(date -Is) ${t1}-${t0}=$((t1 - t0))s" >"$STAMPS/$name"
    printf '%s\t%s\t%s\t%s GB free\n' "$name" "$((t1 - t0))" "$(date -Is)" "$(free_gb)" >>"$LOGS/timings.tsv"
    log "DONE  $name in $((t1 - t0)) s ($(free_gb) GB free)"
  else
    log "FAIL  $name -- last log lines:"; tail -25 "$LOGS/$name.log"
    exit 1
  fi
}

# --- environment files ------------------------------------------------------
write_env() {
  cat >"$SCRIPTS/of-env.sh" <<EOF
# Source this to get the OpenFOAM-v2506 (ESI, dummy Pstream, no MPI) environment.
# Written by scripts/upstream/build-esi-v2506-and-genfoam.sh (do not hand-edit).
export FOAM_INST_DIR=$ROOT
# FOAM_USER_* must be /root/OpenFOAM/user-v2506 as before the reset: the ESI bashrc
# uses \${USER:-user}, so pin it.
export USER=user
set +u
. $ROOT/OpenFOAM-v2506/etc/bashrc WM_MPLIB=dummy WM_COMPILE_OPTION=Opt WM_LABEL_SIZE=32 WM_PRECISION_OPTION=DP
# Parallel compile jobs: honour a caller-set value, default 2 (shared machine).
export WM_NCOMPPROCS=\${UPSTREAM_NCOMPPROCS:-\${WM_NCOMPPROCS:-${N}}}
export OMP_NUM_THREADS=1
# Real solver binaries built from scratch copies (sonicFoam, rhoPimpleFoam, ...).
case ":\$PATH:" in *":$ROOT/gh1363/bin:"*) ;; *) export PATH="$ROOT/gh1363/bin:\$PATH" ;; esac
EOF
  cat >"$SCRIPTS/with-of.sh" <<EOF
#!/bin/bash
# with-of.sh <command...>: run a command inside the OpenFOAM-v2506 (ESI) environment.
# e.g.  with-of.sh blockMesh      WM_NCOMPPROCS=2 with-of.sh wmake -j
# (a WM_NCOMPPROCS set by the caller wins; the default is the build default, $N)
# Written by scripts/upstream/build-esi-v2506-and-genfoam.sh (do not hand-edit).
export UPSTREAM_NCOMPPROCS="\${WM_NCOMPPROCS:-$N}"
# the OpenFOAM bashrc returns non-zero on some shells; do not abort on it
. $ROOT/scripts/of-env.sh >/dev/null 2>&1
exec "\$@"
EOF
  chmod +x "$SCRIPTS/with-of.sh"
}

load_env() {
  set +eu
  # shellcheck disable=SC1090
  UPSTREAM_NCOMPPROCS=$N . "$SCRIPTS/of-env.sh" >/dev/null 2>&1
  set -u
}

# --- prerequisites ----------------------------------------------------------
stage_prereqs() {
  local missing=() t
  for t in gcc g++ make flex bison curl git tar gzip; do command -v "$t" >/dev/null || missing+=("$t"); done
  [ -f /usr/include/zlib.h ] || missing+=(zlib1g-dev)
  [ -f /usr/include/FlexLexer.h ] || missing+=(libfl-dev)
  [ -x /usr/bin/time ] || missing+=(time)   # /usr/bin/time -v in the cfMesh run scripts
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "missing: ${missing[*]}"
    if [ "$(id -u)" = 0 ] && command -v apt-get >/dev/null; then
      apt-get update -qq
      # tool name -> package name
      local pk=()
      for t in "${missing[@]}"; do case $t in g++) pk+=(g++) ;; *) pk+=("$t") ;; esac; done
      apt-get install -y --no-install-recommends "${pk[@]}" flex libfl-dev zlib1g-dev
    else
      echo "install these first (flex is the one that is usually absent): ${missing[*]}"; return 1
    fi
  fi
  gcc --version | head -1
}

# --- ESI source -------------------------------------------------------------
fetch_one() { # url -> $DL/$ESI_TGZ, resumable
  curl -fsSL --connect-timeout 20 -C - -o "$DL/$ESI_TGZ" "$1"
}
stage_fetch_esi() {
  if ! gzip -t "$DL/$ESI_TGZ" 2>/dev/null; then
    local ok=0 u
    for u in "${ESI_URLS[@]}"; do
      echo "trying $u"
      if retry 3 fetch_one "$u" && gzip -t "$DL/$ESI_TGZ"; then ok=1; break; fi
      echo "mirror failed: $u"; rm -f "$DL/$ESI_TGZ"
    done
    [ "$ok" = 1 ] || { echo "every ESI mirror failed"; return 1; }
  fi
  local got; got=$(sha256sum "$DL/$ESI_TGZ" | cut -d' ' -f1)
  [ "$got" = "$ESI_SHA256" ] || echo "WARNING: sha256 $got differs from the pinned $ESI_SHA256 (checked below by api-info)"
  if [ ! -f "$OF/META-INFO/api-info" ]; then
    rm -rf "$OF"
    tar xzf "$DL/$ESI_TGZ" -C "$ROOT"
  fi
  grep -qx 'api=2506' "$OF/META-INFO/api-info" && grep -qx 'patch=0' "$OF/META-INFO/api-info"
  cat "$OF/META-INFO/api-info"
}

# --- wmake wrappers ---------------------------------------------------------
lib_name_of() { sed -n 's#^LIB *= *\$(FOAM_\(USER_\)\?LIBBIN)/\(lib[A-Za-z0-9_]*\).*#\2#p' "$1/Make/files" | head -1; }

# wm_lib_impl <dir under src> : build one ESI library in-tree
wm_lib_impl() {
  local d=$1 lib
  cd "$OF/src/$d"
  lib=$(lib_name_of .)
  wmakeLnInclude -u . || true
  wmake -j"$N" libso .
  [ -f "$FOAM_LIBBIN/$lib.so" ] || { echo "no $lib.so after building $d"; return 1; }
  ls -l "$FOAM_LIBBIN/$lib.so"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "$OF/build/$WM_OPTIONS/src/$d"
}
wm_lib() { run_stage "lib-${1//\//_}" wm_lib_impl "$1"; }

# wm_app_impl <dir under applications> <installed binary name>
wm_app_impl() {
  local d=$1 exe=$2
  cd "$OF/applications/$d"
  wmake -j"$N" .
  [ -x "$FOAM_APPBIN/$exe" ] || { echo "no $exe after building $d"; return 1; }
  ls -l "$FOAM_APPBIN/$exe"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "$OF/build/$WM_OPTIONS/applications/$d"
}
wm_app() { run_stage "app-$2" wm_app_impl "$1" "$2"; }

# keep_orig_edit <file> <sed-script>: derive file from file.orig (kept once) so a
# re-run never edits an edit; writes a unified diff beside it.
keep_orig_edit() {
  local f=$1 s=$2
  [ -f "$f.orig" ] || cp -p "$f" "$f.orig"
  sed "$s" "$f.orig" >"$f"
  diff -u "$f.orig" "$f" >"$f.diff" || true
}

# --- stage: core ESI libraries, blockMesh, checkMesh -------------------------
core_wmake_tools_impl() {
  cd "$OF"
  wmake/src/Allmake
}
core_openfoam_impl() {
  cd "$OF/src"
  wmakeLnInclude -u OpenFOAM
  wmakeLnInclude -u OSspecific/POSIX
  OpenFOAM/Alltouch -check 2>/dev/null || true
  OSspecific/POSIX/Allwmake      # own preferred target: libo
  Pstream/Allwmake               # dummy Pstream (WM_MPLIB=dummy)
  wmake -j"$N" libso OpenFOAM
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "$OF/build/$WM_OPTIONS/src/OpenFOAM"
}
stage_core() {
  run_stage core-wmake-tools core_wmake_tools_impl
  run_stage core-openfoam core_openfoam_impl
  wm_lib fileFormats
  wm_lib surfMesh
  wm_lib meshTools
  wm_lib finiteVolume
  wm_lib mesh/blockMesh
  wm_lib mesh/extrudeModel
  wm_lib dynamicMesh
  wm_app utilities/mesh/generation/blockMesh blockMesh
  wm_app utilities/mesh/manipulation/checkMesh checkMesh
}

# --- stage: parallel/sampling prerequisites (snappyHexMesh and overset need them)
stage_prereq_libs() {
  wm_lib parallel/decompose/decompositionMethods
  wm_lib parallel/distributed
  wm_lib lagrangian/basic
  wm_lib sampling
  wm_lib parallel/decompose/decompose
  wm_lib parallel/reconstruct/reconstruct
}

# --- stage: the 19-library thermo / turbulence / fvOptions closure ------------
# Order = dependency order (esi_thermo_turbulence_closure_build.md). Unlike the
# first build, TurbulenceModels/compressible is built once, unmodified, after
# radiation/solidThermo exist (the record rebuilt it that way too).
stage_closure() {
  local d
  local DIRS=( \
    thermophysicalModels/specie \
    thermophysicalModels/thermophysicalProperties \
    transportModels/compressible \
    thermophysicalModels/basic \
    TurbulenceModels/turbulenceModels \
    thermophysicalModels/solidSpecie \
    thermophysicalModels/reactionThermo \
    thermophysicalModels/solidThermo \
    thermophysicalModels/SLGThermo \
    thermophysicalModels/radiation \
    transportModels/twoPhaseMixture \
    transportModels/incompressible \
    TurbulenceModels/compressible \
    TurbulenceModels/incompressible \
    dynamicFvMesh \
    overset \
    ODE \
    regionModels/regionModel \
    fvOptions )
  # Headers first: some libraries include a sibling's headers before it is built
  # (specie includes thermophysicalProperties/lnInclude). TurbulenceModels/
  # phaseCompressible is headers-only (GeN-Foam's -I lines); it is not built.
  for d in "${DIRS[@]}" TurbulenceModels/phaseCompressible; do
    ( cd "$OF/src/$d" && wmakeLnInclude -u . ) >>"$LOGS/closure.log" 2>&1
  done
  for d in "${DIRS[@]}"; do wm_lib "$d"; done
}

# --- stage: snappyHexMesh, surfaceFeatureExtract ------------------------------
snappy_app_impl() {
  cd "$OF/applications/utilities/mesh/generation/snappyHexMesh"
  # the dummy-Pstream tree has no kahip/metis/scotch/ptscotch dummies and the serial
  # run does not need them (snappy_hex_mesh_esi_v2506/README.md, "Executable")
  keep_orig_edit Make/options '/\$(FOAM_LIBBIN)\/dummy/d;/-lkahipDecomp/d'
  wmake -j"$N" .
  [ -x "$FOAM_APPBIN/snappyHexMesh" ]
  ls -l "$FOAM_APPBIN/snappyHexMesh"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "$OF/build/$WM_OPTIONS/applications/utilities/mesh/generation/snappyHexMesh"
}
sfe_impl() {
  cd "$OF/applications/utilities/surface/surfaceFeatureExtract"
  wmake -j"$N" libso extractionMethod
  wmake -j"$N" .
  [ -x "$FOAM_APPBIN/surfaceFeatureExtract" ]
  ls -l "$FOAM_APPBIN/surfaceFeatureExtract"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "$OF/build/$WM_OPTIONS/applications/utilities/surface/surfaceFeatureExtract"
}
stage_meshing() {
  wm_lib fvMotionSolver
  wm_lib mesh/snappyHexMesh
  run_stage app-snappyHexMesh snappy_app_impl
  run_stage app-surfaceFeatureExtract sfe_impl
}

# --- stage: solvers -----------------------------------------------------------
# Real solver binaries, built from SCRATCH COPIES with the link lines pruned of
# libraries that are not built here (thermoTools, topoChangerFvMesh,
# atmosphericModels, regionFaModels, finiteArea) and EXE redirected to
# $SOLVBIN, exactly as esi_thermo_turbulence_closure_build.md "Edits" 2 and 3.
# Diffs are written beside each copy (Make/options.diff, Make/files.diff).
PRUNE_SOLVER='/-lthermoTools/d;/-ltopoChangerFvMesh/d;/-latmosphericModels/d;/-lregionFaModels/d;/-lfiniteArea/d;/regionFaModels\/lnInclude/d'
scratch_solver_impl() { # <name> <dir under applications/solvers> [extra sub-lib dir]
  local name=$1 d=$2 sub=${3:-} W=$SCRATCH/$1
  rm -rf "$W"; mkdir -p "$W"
  cp -r "$OF/applications/solvers/$d/." "$W/"
  find "$W" -name 'linux64*' -type d -prune -exec rm -rf {} + 2>/dev/null || true
  cd "$W"
  keep_orig_edit Make/options "$PRUNE_SOLVER"
  # the last pruned -I line may leave a dangling backslash on the previous one: harmless
  keep_orig_edit Make/files "s#\\\$(FOAM_APPBIN)#$SOLVBIN#"
  [ -n "$sub" ] && ( cd "$W/$sub" && wmake -j"$N" libso . )
  wmake -j"$N" .
  [ -x "$SOLVBIN/$name" ]
  ls -l "$SOLVBIN/$name"
  find "$W" -name '*.o' -delete
  rm -rf "$W"/Make/linux64*
}
stage_solvers() {
  run_stage solver-rhoCentralFoam scratch_solver_impl rhoCentralFoam compressible/rhoCentralFoam BCs
  run_stage solver-sonicFoam      scratch_solver_impl sonicFoam compressible/sonicFoam
  run_stage solver-rhoPimpleFoam  scratch_solver_impl rhoPimpleFoam compressible/rhoPimpleFoam
  run_stage solver-pimpleFoam     scratch_solver_impl pimpleFoam incompressible/pimpleFoam
}

# --- GeN-Foam -----------------------------------------------------------------
gf_fetch_impl() {
  if [ "$(git -C "$GF" rev-parse HEAD 2>/dev/null)" = "$GF_SHA" ]; then echo "GeN-Foam already at $GF_SHA"; return 0; fi
  [ -d "$GF/.git" ] || { mkdir -p "$GF"; git init -q "$GF"; git -C "$GF" remote add origin "$GF_URL"; }
  # shallow fetch of the pinned commit; fall back to a full fetch if the host refuses by-SHA
  retry 4 git -C "$GF" fetch -q --depth 1 origin "$GF_SHA" || retry 4 git -C "$GF" fetch -q origin
  git -C "$GF" checkout -q -f --detach "$GF_SHA"
  [ "$(git -C "$GF" rev-parse HEAD)" = "$GF_SHA" ]
  git -C "$GF" log -1 --format='%H %s'
}
stage_fetch_genfoam() { run_stage fetch-genfoam gf_fetch_impl; }

# Link-line pruning (libraries that are not built here and that the cases run so
# far do not need): thermoTools, genericPatchFields (needs finiteArea), the
# thermo-mechanics / offbeat / sciantix / OFSolvers libraries. Only Make/options
# link lines are changed (never a .C or .H); -g is dropped from FFSSrc2 (debug info only).
GF_PRUNE='/-lthermoTools\([ \\]\|$\)/d;/-lgenericPatchFields\([ \\]\|$\)/d;/-lThermoMechanicsSolvers\([ \\]\|$\)/d;/-loffbeatMain\([ \\]\|$\)/d;/-loffbeat\([ \\]\|$\)/d;/-lsciantix\([ \\]\|$\)/d;/-lOFSolvers\([ \\]\|$\)/d'
gf_lib_impl() { # <dir under src/classes> [extra sed]
  local d=$1 extra=${2:-}
  cd "$GF/src/classes/$d"
  keep_orig_edit Make/options "$GF_PRUNE${extra:+;$extra}"
  wmakeLnInclude -u .
  wmake -j"$N" libso .
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "Make/$WM_OPTIONS"
}
gf_exe_impl() {
  cd "$GF/GeN-Foam"
  keep_orig_edit Make/options "$GF_PRUNE"
  wmake -j"$N" .
  [ -x "$FOAM_USER_APPBIN/GeN-Foam" ]
  ls -l "$FOAM_USER_APPBIN/GeN-Foam"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "Make/$WM_OPTIONS"
}
stage_genfoam() {
  run_stage gf-libCommon             gf_lib_impl common
  run_stage gf-libMultiRegion        gf_lib_impl multiRegion
  run_stage gf-libNeutronicsSolvers  gf_lib_impl neutronics
  run_stage gf-libFFSSrc2            gf_lib_impl thermalHydraulics/src '/^    -g \\$/d'
  run_stage gf-libFFSSolvers2        gf_lib_impl thermalHydraulics/solvers
  run_stage gf-GeN-Foam              gf_exe_impl
}

# --- cfMesh (gh:#1385) ----------------------------------------------------------
# Edits to upstream: only the committed v2506 API patch (is_contiguous trait, List
# from SLList, demandDrivenData.H includes); no algorithm is touched.
cf_fetch_impl() {
  if [ ! -d "$CF/.git" ]; then
    mkdir -p "$CF"; git init -q "$CF"; git -C "$CF" remote add origin "$CF_URL"
  fi
  if [ "$(git -C "$CF" rev-parse HEAD 2>/dev/null)" != "$CF_SHA" ]; then
    # shallow fetch of the pinned commit; fall back to a full fetch if the host refuses by-SHA
    retry 4 git -C "$CF" fetch -q --depth 1 origin "$CF_SHA" || retry 4 git -C "$CF" fetch -q origin
    git -C "$CF" checkout -q -f --detach "$CF_SHA"
  fi
  [ "$(git -C "$CF" rev-parse HEAD)" = "$CF_SHA" ]
  # idempotent patch: applied already when the reverse applies cleanly
  if git -C "$CF" apply --reverse --check "$CF_PATCH" 2>/dev/null; then
    echo "cfMesh patch already applied"
  else
    git -C "$CF" apply --check "$CF_PATCH"
    git -C "$CF" apply "$CF_PATCH"
  fi
  git -C "$CF" log -1 --format='%H %s'
}
stage_fetch_cfmesh() { run_stage fetch-cfmesh cf_fetch_impl; }

cf_lib_impl() {
  cd "$CF"
  wmake -j"$N" libso meshLibrary
  [ -f "$FOAM_LIBBIN/libmeshLibrary.so" ] || { echo "no libmeshLibrary.so"; return 1; }
  ls -l "$FOAM_LIBBIN/libmeshLibrary.so"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf meshLibrary/Make/"$WM_OPTIONS"
}
cf_exe_impl() { # <executable>
  cd "$CF"
  wmake -j"$N" "executables/$1"
  [ -x "$FOAM_APPBIN/$1" ] || { echo "no $1 binary"; return 1; }
  ls -l "$FOAM_APPBIN/$1"
  [ "$KEEP_OBJECTS" = 1 ] || rm -rf "executables/$1/Make/$WM_OPTIONS"
}
stage_cfmesh() {
  # OMP_NUM_THREADS=1 is set by of-env.sh; meshLibrary compiles with -fopenmp, so
  # every run of these binaries must keep it at 1 (a threaded run reorders reductions).
  run_stage cf-meshLibrary cf_lib_impl
  for e in cartesianMesh cartesian2DMesh tetMesh pMesh; do
    run_stage "cf-$e" cf_exe_impl "$e"
  done
}

# --- verification -------------------------------------------------------------
verify_impl() {
  local V=$ROOT/runs/verify_cavity T=$OF/tutorials/incompressible/icoFoam/cavity/cavity
  local fail=0
  rm -rf "$V"; mkdir -p "$V"
  cp -r "$T/." "$V/"
  cd "$V"
  # laminar pimpleFoam cavity (icoFoam's mesh and BCs; PIMPLE dictionary and schemes added)
  cat >constant/turbulenceProperties <<'EOT'
FoamFile { version 2.0; format ascii; class dictionary; object turbulenceProperties; }
simulationType laminar;
EOT
  echo 'transportModel Newtonian;' >>constant/transportProperties
  cat >system/fvSchemes <<'EOT'
FoamFile { version 2.0; format ascii; class dictionary; object fvSchemes; }
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes { default none; div(phi,U) Gauss linear; div((nuEff*dev2(T(grad(U))))) Gauss linear; }
laplacianSchemes { default Gauss linear orthogonal; }
interpolationSchemes { default linear; }
snGradSchemes { default orthogonal; }
EOT
  cat >system/fvSolution <<'EOT'
FoamFile { version 2.0; format ascii; class dictionary; object fvSolution; }
solvers
{
    p { solver PCG; preconditioner DIC; tolerance 1e-06; relTol 0.05; }
    pFinal { $p; relTol 0; }
    "U.*" { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-05; relTol 0; }
}
PIMPLE { nOuterCorrectors 1; nCorrectors 2; nNonOrthogonalCorrectors 0; pRefCell 0; pRefValue 0; }
EOT
  sed -i 's/^application .*/application pimpleFoam;/; s/^endTime .*/endTime 0.1;/; s/^deltaT .*/deltaT 0.005;/; s/^writeInterval .*/writeInterval 20;/' system/controlDict
  blockMesh > log.blockMesh 2>&1 && tail -2 log.blockMesh
  pimpleFoam > log.pimpleFoam 2>&1 && tail -3 log.pimpleFoam
  grep -q '^End' log.pimpleFoam || { echo "FAIL: pimpleFoam cavity did not finish"; fail=1; }
  helpcheck() { local rc; echo "--- $* -help"; "$@" -help >help.out 2>&1; rc=$?; head -3 help.out; [ $rc = 0 ] || { echo "FAIL: exit $rc"; fail=1; }; }
  helpcheck snappyHexMesh
  helpcheck surfaceFeatureExtract
  helpcheck checkMesh
  helpcheck GeN-Foam
  for s in sonicFoam rhoPimpleFoam rhoCentralFoam; do helpcheck "$SOLVBIN/$s"; done
  rm -rf "$V"/0.[0-9]* "$V"/postProcessing help.out   # run output, not needed
  return $fail
}
stage_verify() { FORCE=1 run_stage verify verify_impl; }

# --- status -------------------------------------------------------------------
status() {
  echo "ROOT=$ROOT  WM_NCOMPPROCS=$N"; echo "stamps:"; ls "$STAMPS" 2>/dev/null | sed 's/^/  /'
  echo "timings:"; cat "$LOGS/timings.tsv" 2>/dev/null | sed 's/^/  /'
  df -h / | tail -1; du -sh "$ROOT" /root/OpenFOAM 2>/dev/null
}

# --- main ---------------------------------------------------------------------
STAGES=(prereqs fetch-esi env core prereq-libs closure meshing solvers fetch-genfoam genfoam fetch-cfmesh cfmesh verify)
[ "$#" -gt 0 ] || set -- "${STAGES[@]}"
T0=$(date +%s)
for s in "$@"; do
  case $s in
    status) status; exit 0 ;;
    env-only) write_env; log "wrote $SCRIPTS/with-of.sh and of-env.sh"; exit 0 ;;
    prereqs)      run_stage prereqs stage_prereqs ;;
    fetch-esi)    run_stage fetch-esi stage_fetch_esi ;;
    env)          write_env; load_env ;;
    core)         write_env; load_env; stage_core ;;
    prereq-libs)  load_env; stage_prereq_libs ;;
    closure)      load_env; stage_closure ;;
    meshing)      load_env; stage_meshing ;;
    solvers)      load_env; stage_solvers ;;
    fetch-genfoam) stage_fetch_genfoam ;;
    genfoam)      load_env; stage_genfoam ;;
    fetch-cfmesh) stage_fetch_cfmesh ;;
    cfmesh)       load_env; stage_fetch_cfmesh; stage_cfmesh ;;
    verify)       load_env; stage_verify ;;
    *) echo "unknown stage: $s"; exit 64 ;;
  esac
done
log "ALL REQUESTED STAGES DONE in $(( $(date +%s) - T0 )) s; $(free_gb) GB free"
