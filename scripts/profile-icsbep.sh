#!/usr/bin/env bash
# Profile the four ICSBEP criticality benchmarks on the HIGH (continuous-energy
# ENDF/B-VIII.0) data path: Godiva (HEU-MET-FAST-001), Jemima
# (IEU-MET-FAST-002), HST-009 (HEU-SOL-THERM-009) and LCT-008
# (LEU-COMP-THERM-008).
#
#   scripts/profile-icsbep.sh flame    [out-dir] [case ...]   # perf + inferno SVGs
#   scripts/profile-icsbep.sh callgrind [out-dir] [case ...]  # valgrind callgrind
#
# Cases: godiva jemima hst009 lct008 (default: all four).
# LCT008_ARGS: extra arguments for lct008_keff; default --cheap-nuclides
# (the full tape set runs out of memory at Fe-57 on 16 GB, GitHub #339).
#
# flame: `perf record` with the software `cpu-clock` event, because hardware
# PMUs are often absent in VMs; frame-pointer stacks. Each run is the
# example's DEFAULT configuration, so the profile is of the real benchmark.
# Output per case: <case>.perf.data, <case>.folded, <case>.svg, <case>.log.
#
# callgrind: instruction counts of the TRANSPORT phase only, exact but
# ~50-100x slower. Data preparation runs uninstrumented; collection starts
# at the "histories/gen" line. Reduced statistics (CG_NPART, CG_NINACTIVE,
# CG_NACTIVE; default 500 x [20 + 5]) so it finishes: it measures the cost
# structure per history, not the eigenvalue. Output: <case>.callgrind.out,
# <case>.callgrind.txt (inclusive), <case>.callgrind.self.txt, <case>.log.
#
# Needs: perf (linux-tools), inferno (`cargo install inferno`), valgrind.
# Builds with the workspace `profiling` profile (release + line tables) and
# frame pointers, into target/profiling/.
set -euo pipefail

mode="${1:?usage: profile-icsbep.sh flame|callgrind [out-dir] [case ...]}"
out="${2:-target/profile-icsbep}"
shift $(( $# >= 2 ? 2 : $# ))
cases=("$@")
[ ${#cases[@]} -eq 0 ] && cases=(godiva jemima hst009 lct008)

root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$root"
mkdir -p "$out"

# Prefer the real binary: Ubuntu's /usr/bin/perf is a wrapper that refuses to
# run when linux-tools does not match the running kernel (as in most VMs).
PERF="${PERF:-$(ls /usr/lib/linux-tools/*/perf 2>/dev/null | head -1)}"
PERF="${PERF:-$(command -v perf)}"

example_of() {
  case "$1" in
    godiva) echo godiva_keff_endf_local ;;
    jemima) echo jemima_keff ;;
    hst009) echo hst009_keff ;;
    lct008) echo lct008_keff ;;
    *) echo "unknown case $1" >&2; exit 2 ;;
  esac
}

examples=()
for c in "${cases[@]}"; do examples+=(--example "$(example_of "$c")"); done

RUSTFLAGS="${RUSTFLAGS:-} -C force-frame-pointers=yes" \
  cargo build --profile profiling -p outram-mc-libs --features endf-pebble-cases "${examples[@]}"

bin() { echo "target/profiling/examples/$(example_of "$1")"; }

# Extra arguments for LCT-008 only. Profiling uses `--cheap-nuclides` by
# default (maintainer direction, 2026-09-27): the example's own default tape
# set includes Fe-57, whose LRF=7 reconstruction exhausts 16 GB (GitHub #339),
# so the default run cannot finish on a 16 GB machine. Set LCT008_ARGS= (empty)
# to profile the full tape set on a machine that can hold it.
read -r -a lct_extra <<< "${LCT008_ARGS---cheap-nuclides}"
extra_of() { [ "$1" = lct008 ] && printf '%s\n' "${lct_extra[@]}"; true; }

for c in "${cases[@]}"; do
  case "$mode" in
    flame)
      [ -x "$PERF" ] || { echo "perf not found; set PERF=" >&2; exit 2; }
      t0=$(date +%s)
      "$PERF" record -e cpu-clock -F 499 -g -o "$out/$c.perf.data" \
        -- "$(bin "$c")" $(extra_of "$c") > "$out/$c.log" 2> "$out/$c.stderr"
      echo "wall_s $(( $(date +%s) - t0 ))" > "$out/$c.time"
      "$PERF" script -i "$out/$c.perf.data" 2>/dev/null \
        | inferno-collapse-perf > "$out/$c.folded"
      inferno-flamegraph --minwidth 1 --title "$(example_of "$c") (perf cpu-clock, profiling build)" \
        < "$out/$c.folded" > "$out/$c.svg"
      ;;
    callgrind)
      # TRANSPORT ONLY, at reduced statistics. Nuclear-data preparation runs
      # with instrumentation off (--instr-atstart=no, a few x native instead
      # of ~50-100x); instrumentation is switched on when the example prints
      # its "histories/gen" line, which every one of the four prints just
      # before transport. Data preparation is profiled by the flame mode.
      # Enough generations that the switch-on (below) lands inside transport:
      # at 500 x [2 + 3] Godiva's transport took 0.6 s under valgrind and was
      # over before a 2-second poll noticed it, so NOTHING was collected.
      np="${CG_NPART:-500}"; ni="${CG_NINACTIVE:-20}"; na="${CG_NACTIVE:-5}"
      args=()
      [ "$c" = lct008 ] && args=(--particles "$np" --inactive "$ni" --active "$na" $(extra_of lct008))
      OUTRAM_NPART="$np" OUTRAM_NINACTIVE="$ni" OUTRAM_NACTIVE="$na" \
        valgrind --tool=callgrind --instr-atstart=no \
        --callgrind-out-file="$out/$c.callgrind.out" \
        "$(bin "$c")" "${args[@]}" > "$out/$c.log" 2>&1 &
      pid=$!
      until grep -q "histories/gen" "$out/$c.log" 2>/dev/null; do
        kill -0 "$pid" 2>/dev/null || { echo "$c exited before transport" >&2; break; }
        sleep 0.1
      done
      callgrind_control -i on "$pid" > "$out/$c.toggle.log" 2>&1 \
        || echo "$c: callgrind_control failed, see $c.toggle.log" >&2
      wait "$pid"
      # Refuse an empty profile rather than report one.
      if ! grep -q "^summary: *[1-9]" "$out/$c.callgrind.out"; then
        echo "$c: callgrind collected nothing (transport ended before the switch-on?)" >&2
        exit 3
      fi
      callgrind_annotate --inclusive=yes "$out/$c.callgrind.out" > "$out/$c.callgrind.txt"
      callgrind_annotate "$out/$c.callgrind.out" > "$out/$c.callgrind.self.txt"
      ;;
    *) echo "mode must be flame or callgrind" >&2; exit 2 ;;
  esac
  echo "done: $c"
done
