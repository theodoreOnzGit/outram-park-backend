#!/bin/sh
# OpenMC seeds 17-24 at 1.63576 and 1.75, same deck, library and settings as ensemble_550/run_openmc_ensemble.sh
S=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad
D=/home/user/outram-park-backend/.claude/worktrees/agent-a548649e5eb955778/crates/outram-mc-libs/verification_and_validation/tutorial_rung4
export OMP_NUM_THREADS=2
for seed in $(seq 17 24); do
 for p in 1.63576 1.75; do
  nice -n 10 $S/venv-openmc/bin/python $D/openmc_inputs/lct008_pin_cell_openmc.py --pitch $p \
    --xs $S/openmc-libs/lct008_pin11/h5_njoy/cross_sections.xml --seed $seed --threads 2 \
    --particles 5000 --inactive 50 --active 200 --csv $D/remeasure3_550/openmc_extra.csv \
    --workdir /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/omc/p${p}_s$seed --openmc $S/openmc-install/bin/openmc >> /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/omc/stdout.log 2>&1
  rm -f /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/omc/p${p}_s$seed/statepoint.*.h5
 done
done
echo DONE >> /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/omc/stdout.log
