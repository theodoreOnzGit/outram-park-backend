#!/usr/bin/env bash
# The gh:#1825 references: every (input, switches) run of
# tests/tetgen_sizing_vs_upstream.rs, with the UNMODIFIED upstream binary
# (the files) and the instrumented one (the tg5 / tg6 / tgm counters).
#
#   run_reference.sh <inputs dir> <reference dir>
#
# <inputs dir> holds what `write_sizing_inputs` writes (TETGEN_SIZING_INPUTS).
# TETGEN_UPSTREAM: the unmodified build dir (default /home/user/tetgen-upstream/build);
# TETGEN_INSTR: the dir of the instrumented `tetgen` (stage 4, 5, 6 patches
# and sizing_reach.patch applied). ONLY="<input>:<sw> ..." runs a subset.
set -euo pipefail
I=$1; R=$2
U=${TETGEN_UPSTREAM:-/home/user/tetgen-upstream/build}
X=${TETGEN_INSTR:?set TETGEN_INSTR to the instrumented tetgen directory}
mkdir -p "$R"
RUNS="example_grad:pqmz box_hole_grad:pqmz comb_prism_grad:pqmz slit_box_grad:pqmz skew_points_grad:pqmz sphere_grad:pqmz example_holes:pqmz fuel_pin_hole_grad:pqmz
skew_points_coarse:pmz skew_points_coarse:pqmz skew_segments_coarse:pmz skew_segments_coarse:pqmz facet_wall_coarse:pmz facet_wall_coarse:pqmz
example_grad:pmz example_grad:pqm2z example_grad:pq1.414a0.01mz example_grad:pz example_grad:pqz fuel_pin_regions_grad:pqAmz
skew_points_m1_grad:pmz skew_points_m1_grad:pqmz
pipe_grad:pq1.414a0.0026mz fuel_channel_grad:pq1.414a0.0026mz pebble_grad:pq1.414a0.0026mz
pipe_graded:pq1.414a0.0147mz fuel_channel_graded:pq1.414a0.00436mz pebble_graded:pq1.414a0.00436mz"
RUNS=${ONLY:-$RUNS}
W=$(mktemp -d)
mkdir -p "$W/u" "$W/i"
for r in $RUNS; do
  n=${r%%:*}; sw=${r#*:}; t=${sw//\//_}
  if [ -f "$I/$n.poly" ]; then ext=poly; else ext=smesh; fi
  for d in u i; do find "$W/$d" -type f -delete; cp "$I/$n".* "$W/$d/"; done
  ( cd "$W/u" && PATH="$U:$PATH" tetgen "-$sw" "$n.$ext" > log 2>&1 )
  ( cd "$W/i" && TG_REACH5=r5 TG_REACH6=r6 TG_REACHM=rm PATH="$X:$PATH" tetgen "-$sw" "$n.$ext" > log 2>&1 )
  for e in node ele face edge mtr p2t; do
    if [ -f "$W/u/$n.1.$e" ]; then
      cp "$W/u/$n.1.$e" "$R/$n.$t.$e"
      cmp -s "$W/u/$n.1.$e" "$W/i/$n.1.$e" || echo "INSTRUMENTED BUILD DIFFERS: $n -$sw .$e"
    fi
  done
  cp "$W/i/r5" "$R/$n.$t.reach5"; cp "$W/i/r6" "$R/$n.$t.reach6"; cp "$W/i/rm" "$R/$n.$t.reachm"
  echo "$n -$sw: $(head -1 "$W/u/$n.1.node" | awk '{print $1}') nodes, $(head -1 "$W/u/$n.1.ele" | awk '{print $1}') tets"
done
