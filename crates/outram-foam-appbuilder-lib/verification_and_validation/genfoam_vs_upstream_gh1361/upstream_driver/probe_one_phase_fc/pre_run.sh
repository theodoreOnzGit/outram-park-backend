#!/bin/bash
# run_case.sh PRE_RUN hook for the gh:#1894 probe: tight protocol, then the isothermal/no-exchanger control when ISO=1
export TIGHT=1 PSOLVER=pcg TOL=${TOL:-1e-30} END_TIME=${END_TIME:-0.001}
bash /home/user/outram-park-backend/.claude/worktrees/agent-a4ad2e0194b2cf44d/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/scripts/fc_protocol.sh
if [ -n "$ISO" ]; then
  python3 -I - <<'EOF'
import re
p='constant/fluidRegion/phaseProperties'
s=open(p).read()
s=s.replace('powerDensity    1.1e9;','powerDensity    0;')
i=s.index('heatExchangers')
j=s.index('{',i)
d=0;k=j
while True:
    if s[k]=='{': d+=1
    if s[k]=='}':
        d-=1
        if d==0: break
    k+=1
s=s[:i]+s[k+1:]
open(p,'w').write(s)
EOF
fi
