#!/bin/sh
# gh:#1635 item 2: per-build mean, seed-scatter sem, and new - old with z,
# from the K_ONLY lines of htr10_deterministic_vs_mc (awk, no Python).
# Run from this folder: sh dvm_stats.sh
st(){ awk '/K_ONLY/{k=$8;s+=k;ss+=k*k;n++;e+=$10*$10}END{m=s/n;sd=sqrt((ss-n*m*m)/(n-1));printf "%d %.6f %.6f %.6f\n",n,m,sd/sqrt(n),sqrt(e)/n}' "$@"; }
O=$(st nee_soon_dvm_old_konly_*.out); N=$(st nee_soon_dvm_new_konly_*.out)
echo "old n mean sem_scatter sem_quoted: $O"; echo "new n mean sem_scatter sem_quoted: $N"
echo "$O $N" | awk '{d=($6-$2)*1e5; s=sqrt($3^2+$7^2)*1e5; printf "new-old = %.0f +/- %.0f pcm, z=%.2f\n",d,s,d/s}'
