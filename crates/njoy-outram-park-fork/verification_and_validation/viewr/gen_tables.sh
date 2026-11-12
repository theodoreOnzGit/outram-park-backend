#!/bin/bash
# Generate the Rust font and colour tables from graph.f90 (no hand typing).
U=/home/user/outram-park-backend/.claude/worktrees/agent-a32ca0b67105882a6/crates/njoy-outram-park-fork/upstream_source/NJOY2016/src/graph.f90
echo "// @generated from NJOY2016.80 src/graph.f90 by scripts (gh:#1369); do not edit by hand."
for n in 1 2 3 4 5 6 7 8 9 10 11 12; do
  s=$(grep -n "dimension(32)::cw$n=" $U | cut -d: -f1)
  e=$(awk -v s=$s 'NR>=s && /\/\)/ {print NR; exit}' $U)
  vals=$(sed -n "${s},${e}p" $U | sed 's/^.*=(\/&*//; s/\/)//' | grep -o '[0-9]*\.[0-9]*e0_kr\|[0-9]\.e0_kr' | sed 's/e0_kr//' | sed 's/^\./0./; s/\.$/.0/' | tr '\n' ',' | sed 's/,$//')
  echo "/// \`cw$n\` of graph.f90."
  echo "pub const CW$n: [f64; 32] = [$vals];"
done
for spec in "ibrgb:IBRGB:8" "ifrgb:IFRGB:9" "isrgb:ISRGB:40"; do
  name=${spec%%:*}; rest=${spec#*:}; up=${rest%%:*}; cnt=${rest#*:}
  s=$(grep -n "dimension(3,$cnt)::$name=" $U | cut -d: -f1)
  e=$(awk -v s=$s 'NR>=s && /\(\/3,/ {print NR; exit}' $U)
  vals=$(sed -n "${s},${e}p" $U | sed 's/!.*//; s/^.*reshape((\///; s/\/),(\/3,.*//; s/&//g' | grep -o '[0-9][0-9]*' | tr '\n' ',' | sed 's/,$//')
  echo "/// \`$name\` of graph.f90: \`[(r, g, b); $cnt]\`, flattened column-major as in the Fortran."
  echo "pub const $up: [i32; $((cnt*3))] = [$vals];"
done
