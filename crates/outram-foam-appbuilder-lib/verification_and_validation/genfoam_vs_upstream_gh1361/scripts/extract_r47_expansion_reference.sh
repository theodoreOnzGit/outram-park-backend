#!/bin/bash
# gh:#1898 (README round 47, item 2c): refs/r47_expansion.ref.txt.gz from the dumpXSexp runs of make_r47_expansion_case.sh
# (/home/user/upstream-build/gh1887/runs/{ea,eb,ec}/xsexp). Format: "CASE <name>" then "ARR <array> <one value per cell>".
HERE="$(cd "$(dirname "$0")/.." && pwd)"
S=$(mktemp)
for v in ea eb ec; do
  echo "CASE $v" >> "$S"
  D=/home/user/upstream-build/gh1887/runs/$v/xsexp
  for a in dispX dispY dispZ axExp radExp D0 D1 nuSigmaEff0 nuSigmaEff1 sigmaRemoval0 sigmaRemoval1 sigmaFromTo_0_1 discFactor0 discFactor1; do
    echo "ARR $a $(tr '\n' ' ' < $D/$a)" >> "$S"
  done
done
gzip -9 -n -c "$S" > "$HERE/refs/r47_expansion.ref.txt.gz"
rm -f "$S"
ls -l "$HERE/refs/r47_expansion.ref.txt.gz"
