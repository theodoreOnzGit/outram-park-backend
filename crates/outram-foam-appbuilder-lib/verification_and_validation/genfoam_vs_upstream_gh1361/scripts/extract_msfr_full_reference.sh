#!/bin/bash
# gh:#1813 round 22: refs/<name>.ref.txt.gz from a full `2D_MSFR` transient run of the real GeN-Foam binary
# (run_msfr_chain.sh transient, PROTOCOL=tight, every step written, writePrecision 17), log + reaped fluid T.
# usage: extract_msfr_full_reference.sh <run root>/transient <reaped T dir> <output name> "<steps whose T is kept>"
# Output (one item per number, space separated; per coupling iteration where a step has several):
#   N                 number of steps (`Time = ` lines)
#   DT                the `deltaT = ` printed after each step
#   CPITER_n          coupling iterations of step n (`Solving neutronics` count);  CONV_n: 1 if "Multiphysics loop converged"
#   HRES_n            every `Solving for h` initial residual of step n, in log order
#   TSTAT_n           every `T (avg min max)` triple of step n, in log order
#   CO_n              the last Courant line (mean max) of step n
#   POWER_n           `power:` of each coupling iteration;  NITER_n: neutron iterations of each;  NRESF_n: final neutroResidual of each
#   S2MAX_n           per coupling iteration: max over the SECOND solve of every flux group, every neutron iteration, of
#                     `Initial residual` (fluxEq.H:29, the solve inside `residual_ = max(neutroEq.solve()...initialResidual(), residual_)`)
#   RESID_n           per coupling iteration: upstream's `residual_` = max(S2MAX, NRESF) (solveNeutronics.H:138)
#   T_<k>             fluidRegion/T after step k, for k in the list
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=$1; TD=$2; NAME=$3; KEEP=$4
L=$R/log.GeN-Foam
OUT=$(mktemp)
awk '
  /^Time = /{s++; ci=0}
  /^deltaT = /{dt[++nd]=$3}
  /^Solving neutronics/{ci++; cp[s]=ci; fc[s,ci]=0; s2[s,ci]=0}
  /Solving for h,/{split($0,a,"Initial residual = "); split(a[2],b,","); hr[s]=hr[s] " " b[1]}
  /^T \(avg min max\)/{ts[s]=ts[s] " " $6" "$7" "$8}
  /^Courant Number/{co[s]=$6" "$7}
  /Solving for fluxStar[0-9]*,/{
      split($0,a,"Solving for "); split(a[2],g,","); k=g[1]; cnt[k]++;
      if (cnt[k]%2==0){ split($0,a,"Initial residual = "); split(a[2],b,","); if (b[1]+0>s2[s,ci]) s2[s,ci]=b[1]+0 } }
  /^Solving neutronics/{for(k in cnt) cnt[k]=0}
  /^Final neutroResidual at neutroIter no/{split($0,a,"no "); split(a[2],b,":"); ni[s]=ni[s] " " b[1]; split(b[2],c," "); nr[s]=nr[s] " " c[1]; fin[s,ci]=c[1]+0}
  /^power: /{pw[s]=pw[s] " " $2}
  /^Multiphysics loop converged/{cv[s]=1}
  END{
    printf "N %d\n", s; printf "DT"; for(i=1;i<=s;i++) printf " %s", dt[i]; printf "\n";
    for(i=1;i<=s;i++){
      printf "CPITER_%d %d\nCONV_%d %d\nHRES_%d%s\nTSTAT_%d%s\nCO_%d %s\nPOWER_%d%s\nNITER_%d%s\nNRESF_%d%s\n", i, cp[i], i, cv[i]+0, i, hr[i], i, ts[i], i, co[i], i, pw[i], i, ni[i], i, nr[i];
      printf "S2MAX_%d", i; for(j=1;j<=cp[i];j++) printf " %.17g", s2[i,j]; printf "\n";
      printf "RESID_%d", i; for(j=1;j<=cp[i];j++){ m=s2[i,j]; if (fin[i,j]>m) m=fin[i,j]; printf " %.17g", m }; printf "\n";
    } }' "$L" > "$OUT"
for k in $KEEP; do echo "T_$k $(cat $TD/T_$k)" >> "$OUT"; done
gzip -9 -n -c "$OUT" > "$HERE/refs/$NAME.ref.txt.gz"
rm -f "$OUT"
ls -l "$HERE/refs/$NAME.ref.txt.gz"
