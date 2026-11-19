#!/bin/sh
# gh:#1636: recompute the step 0/1/4 statistics from the raw outputs in this folder.
# usage: sh analyse_1636.sh   (run from this directory). awk only.
ex() { grep -h " $1 *seed" "$2" | sed -E 's/.*kfull = ([0-9.]+) .*/\1/'; }
stat() { # name, file with one k per line
  awk -v nm="$1" '{x[NR]=$1; s+=$1} END {m=s/NR; for(i=1;i<=NR;i++) v+=(x[i]-m)^2;
    printf "%-34s n=%d mean %.5f sd %.0f pcm sem %.0f pcm\n", nm, NR, m, sqrt(v/(NR-1))*1e5, sqrt(v/(NR-1)/NR)*1e5}' "$2"
}
T=${TMPDIR:-/tmp}
ex MOVING step0_default.out > $T/m0; ex MOVING step1_scatter.out > $T/m1
ex MOVING step4_moving.out > $T/m4; ex AT-REST step4_atrest.out > $T/ar
stat "step0 MOVING default" $T/m0
stat "step1 MOVING legacy scatter" $T/m1
stat "step4 MOVING no resonance scat." $T/m4
stat "AT-REST (identical both builds)" $T/ar
pair() { # name fileA fileB  -> mean(B-A) +- sem in pcm, unpaired too
  paste "$2" "$3" | awk -v nm="$1" '{d[NR]=($2-$1)*1e5; a[NR]=$1; b[NR]=$2; sa+=$1; sb+=$2; s+=d[NR]}
    END {n=NR; m=s/n; for(i=1;i<=n;i++){v+=(d[i]-m)^2; va+=(a[i]-sa/n)^2; vb+=(b[i]-sb/n)^2}
    ep=sqrt(v/(n-1)/n); eu=sqrt(va/(n-1)/n+vb/(n-1)/n)*1e5;
    printf "%-44s paired %+.0f +- %.0f (z %.1f, sd %.0f) | unpaired %+.0f +- %.0f (z %.1f)\n", nm, m, ep, m/ep, sqrt(v/(n-1)), (sb-sa)/n*1e5, eu, ((sb-sa)/n*1e5)/eu}'
}
pair "legacy scatter - default (MOVING)" $T/m0 $T/m1
pair "no resonance scat. - default (MOVING)" $T/m0 $T/m4
pair "worth, default (AT-REST - MOVING)" $T/m0 $T/ar
pair "worth, no resonance scat. (AT-REST - MOVING)" $T/m4 $T/ar
