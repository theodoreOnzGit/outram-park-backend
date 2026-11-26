#!/bin/bash
# usage: summ.sh <tag> [ROUND13_OFF list]; writes row files and prints worst-per-range summaries
S=/tmp/claude-0/-home-user-outram-park-backend/7cef3b0b-c239-561b-8dd4-50ffa1885b0f/scratchpad
T=/home/user/outram-park-backend/target/release/deps
cd /home/user/outram-park-backend/.claude/worktrees/agent-a3e3d68ae403e63e3/crates/outram-foam-appbuilder-lib
export ROUND13_OFF=$2 ROUND13_KEEP_GOING=1
b() { ls -t $T/$1-* | grep -v "\.d$\|rmeta\|\.rlib" | head -1; }
echo "### $1 OFF=[$2]"
STEPS=40 $(b genfoam_psbt_trace_vs_upstream) --ignored --nocapture psbt_closed_dump_rows 2>&1 | grep "^step" > $S/r_psbt.txt
echo "closed PSBT"; for r in "1 8" "9 13" "14 40"; do python3 -I $S/worst.py $S/r_psbt.txt $r | sed 's/ T.interface=[^ ]*//; s/ p_rgh=[^ ]*//; s/ alphaRhoMagU[^ ]*//; s/ areaDensity[^ ]*//' | cut -c1-330; done
REFS=psbt_shipped_head,psbt_shipped_s100,psbt_shipped_s200,psbt_shipped_s300,psbt_shipped_s407 STEPS=407 $(b genfoam_psbt_shipped_vs_upstream) --ignored --nocapture psbt_shipped_dump_rows 2>&1 | grep "^step" > $S/r_ship.txt
echo "shipped PSBT (rows: $(wc -l < $S/r_ship.txt))"; for r in "1 12" "13 13" "14 60" "61 407"; do python3 -I $S/worst.py $S/r_ship.txt $r | sed 's/ T.interface=[^ ]*//; s/ p_rgh=[^ ]*//; s/ alphaRhoMagU[^ ]*//; s/ areaDensity[^ ]*//; s/ phi.[a-z]*=[^ ]*//g' | cut -c1-300; done
STEPS=86 $(b genfoam_chf_trace_vs_upstream) --ignored --nocapture chf_closed_dump_rows 2>&1 | grep "^step" > $S/r_chf.txt
echo "constant_chf"; for r in "1 1" "2 6" "7 40" "41 86"; do python3 -I $S/worst.py $S/r_chf.txt $r | sed 's/ T.interface=[^ ]*//; s/ p_rgh=[^ ]*//; s/ alphaRhoMagU[^ ]*//; s/ chfBool5=[^ ]*//; s/ htc.vapour[^ ]*//' | cut -c1-300; done
VARIANT=lookup_table_chf REFS=chf_lookup_head STEPS=120 $(b genfoam_chf_trace_vs_upstream) --ignored --nocapture chf_closed_dump_rows 2>&1 | grep "^step" > $S/r_lookup.txt
echo "lookup_table_chf"; for r in "1 3" "4 6" "7 40" "41 120"; do python3 -I $S/worst.py $S/r_lookup.txt $r | sed 's/ T.interface=[^ ]*//; s/ p_rgh=[^ ]*//; s/ alphaRhoMagU[^ ]*//; s/ chfBool5=[^ ]*//' | cut -c1-300; done
$(b genfoam_two_phase_mules_vs_upstream) --ignored --nocapture mules_dump_rows 2>&1 | grep "DIVERGED\|^step" > $S/r_mules.txt
echo "MULES 1D_boiling"; grep DIVERGED $S/r_mules.txt; for r in "1 70" "560 615" "616 634" "635 635" "636 640"; do python3 -I $S/worst.py $S/r_mules.txt $r | sed 's/ T.interface=[^ ]*//; s/ p_rgh=[^ ]*//; s/ alphaPhi[^ ]*//; s/ phi[^ ]*//g; s/ sub_ok=[^ ]*//' | cut -c1-300; done
