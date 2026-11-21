#!/bin/sh
# gh:#550 OpenMC seed ensemble: seeds 1-16 x pitches, 2 threads, sequential.
S=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad
R=/home/user/outram-park-backend/.claude/worktrees/agent-ab52a25bcbb01c7db/crates/outram-mc-libs/verification_and_validation/tutorial_rung4
export OMP_NUM_THREADS=2
for seed in $(seq 1 16); do
 for p in 1.63576 1.75 2.40; do
  $S/venv-openmc/bin/python $R/openmc_inputs/lct008_pin_cell_openmc.py --pitch $p \
    --xs $S/openmc-libs/lct008_pin11/h5_njoy/cross_sections.xml --seed $seed --threads 2 \
    --particles 5000 --inactive 50 --active 200 --csv $R/ensemble_550/openmc_seeds.csv \
    --workdir $S/run550/omc_p${p}_s$seed --openmc $S/openmc-install/bin/openmc >> $R/ensemble_550/openmc_stdout.log 2>&1
 done
done
echo DONE >> $R/ensemble_550/openmc_stdout.log
