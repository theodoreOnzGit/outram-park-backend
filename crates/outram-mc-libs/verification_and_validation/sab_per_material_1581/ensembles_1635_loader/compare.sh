#!/bin/bash
# usage: compare.sh DIR NSEEDS   (gh:#1635 item 3). Seed-ensemble comparison old vs new,
# shell and awk only. z = (mean_new - mean_old) / sqrt(sem_old^2 + sem_new^2), sem = sd/sqrt(n).
D=${1:-.}; N=${2:-6}
extract() { # file key  -> value (first number after '=')
  awk -v key="$2" '$0 ~ key { for (i = 1; i <= NF; i++) if ($i == "=") { print $(i+1); exit } }' "$1"
}
row() { # label prog key
  label=$1; prog=$2; key=$3
  for arm in old new; do
    : > /tmp/cmp_$arm.$$
    for i in $(seq 1 "$N"); do
      s=$((20261100+i))
      v=$(extract "$D/${prog}_${arm}_seed$s.out" "$key")
      [ -n "$v" ] && echo "$v" >> /tmp/cmp_$arm.$$
    done
  done
  paste -d' ' /tmp/cmp_old.$$ /tmp/cmp_new.$$ | awk -v label="$label" '
    { a[NR]=$1; b[NR]=$2; sa+=$1; sb+=$2 }
    END {
      n=NR; if (n<2) { printf "%-28s n=%d (too few)\n", label, n; exit }
      ma=sa/n; mb=sb/n
      for (i=1;i<=n;i++) { va+=(a[i]-ma)^2; vb+=(b[i]-mb)^2 }
      sda=sqrt(va/(n-1)); sdb=sqrt(vb/(n-1))
      sem=sqrt(sda^2/n + sdb^2/n); z=(sem>0)?(mb-ma)/sem:0
      printf "%-28s n=%d old %.5f (sd %.5f)  new %.5f (sd %.5f)  diff %+.5f +/- %.5f  z %+.2f  %s\n", label, n, ma, sda, mb, sdb, mb-ma, sem, z, (z>2||z<-2)?"MISS":"ok"
    }'
  rm -f /tmp/cmp_old.$$ /tmp/cmp_new.$$
}
echo "== ugraphite_four_factor main case (rung 2), $N seeds per arm"
row "k_inf" uf "k_inf \\(power"
row "eta" uf "^  eta "
row "f" uf "^  f "
row "p (3-group)" uf "^  p "
row "epsilon" uf "^  epsilon "
row "p (energy-only)" uf "p \\(energy-only escape\\)"
row "fold/tally" uf "fold"
echo "== lumped_ugraphite_kinf r = 0.3 cm (rung 3), $N seeds per arm"
row "k_inf" lu "k_inf \\(power"
row "eta" lu "^  eta "
row "f" lu "^  f "
row "p (3-group)" lu "^  p "
row "epsilon" lu "^  epsilon "
