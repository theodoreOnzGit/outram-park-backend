#!/bin/bash
# usage: probe_iso.sh  (port probe vs upstream iso21)
W=/home/user/outram-park-backend/.claude/worktrees/agent-a58cfe254a2c8d14f
cd $W
export CARGO_TARGET_DIR=$W/target-private CARGO_BUILD_JOBS=2
S=/tmp/claude-0/s
rm -f $S/port_iso21.txt
PORT_PROBE_FILE=$S/port_iso21.txt DIAG_NOUT=2 DIAG_NCORR=1 DIAG_P0=1 DIAG_NOHX=1 HX_TOL=1e-30 cargo test --release -p outram-foam-appbuilder-lib --test genfoam_heat_exchanger_vs_upstream diagnostic_step1_one_corrector -- --ignored --nocapture 2>&1 | grep -E "^error" -A8 | head -20
cd /home/user/upstream-build/probe1894
python3 -I cmp.py up_iso21.txt $S/port_iso21.txt | grep -v missing | awk '$NF!="rel=0.00e+00"' | cut -c1-100
echo "---- done"
