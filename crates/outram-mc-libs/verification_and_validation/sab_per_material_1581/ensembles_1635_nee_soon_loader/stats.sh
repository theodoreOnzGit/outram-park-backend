#!/bin/sh
# gh:#1635 nee_soon loader: per-build mean, seed-scatter sem, new - old with z (awk).
# Run from this folder: sh stats.sh
for b in old new; do grep -h -m1 -E "^\s*k_eff\s*=" ${b}_seed*.out | awk -v b=$b '{k=$3;s+=k;ss+=k*k;n++;e+=$5*$5}END{m=s/n;sd=sqrt((ss-n*m*m)/(n-1));printf "%s %d %.6f %.6f %.6f\n",b,n,m,sd/sqrt(n),sqrt(e)/n}'; done > stats.txt
cat stats.txt
awk 'NR==1{o=$3;os=$4}NR==2{d=($3-o)*1e5;s=sqrt(os^2+$4^2)*1e5;printf "new-old = %.0f +/- %.0f pcm, z=%.2f\n",d,s,d/s}' stats.txt
