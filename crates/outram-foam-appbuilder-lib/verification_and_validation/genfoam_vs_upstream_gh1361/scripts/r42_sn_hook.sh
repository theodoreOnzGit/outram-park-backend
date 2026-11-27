#!/bin/bash
# PRE_RUN hook of round 42 (gh:#1887): the round-2 SN slab (tight protocol + sn_slab_protocol.sh) with every discFactor set to $DF.
export CASES=/home/user/outram-park-backend/.claude/worktrees/agent-affbdc8461f783f55/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/cases/sn_slab STEPS=6
bash /home/user/outram-park-backend/.claude/worktrees/agent-affbdc8461f783f55/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/scripts/tight_protocol.sh
bash /home/user/outram-park-backend/.claude/worktrees/agent-affbdc8461f783f55/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/scripts/sn_slab_protocol.sh
sed -i "s/\(discFactor .*List<scalar> [0-9]* \)([0-9.e+-]*)/\1($DF)/" constant/neutroRegion/nuclearData
