#!/bin/bash
# After the previous agent's run_all.sh (pid 2332: Jemima legacy, HST-009 default+legacy at 128 seeds) exits,
# run LCT-008 case 1 default arm at 32 seeds.
cd /home/user/outram-park-backend/.claude/worktrees/agent-acefa2821d57a2cf6/crates/outram-mc-libs
EX=/tmp/claude-0/-home-user-outram-park-backend/208a80c6-8981-41e3-ac0f-1769a3a0e997/scratchpad/target-njoy/release/examples
L=/home/user/outram-park-backend/.claude/worktrees/agent-acefa2821d57a2cf6/remeasure_logs
while kill -0 2332 2>/dev/null; do sleep 30; done
echo "$(date -u -Iseconds) start lct008_keff default 32 seeds (chain2)" >> $L/progress.txt
OUTRAM_BENCH_SEEDS=32 $EX/lct008_keff > $L/lct008_keff_default_s32.log 2>&1
echo "$(date -u -Iseconds) end lct008_keff default rc=$?" >> $L/progress.txt
