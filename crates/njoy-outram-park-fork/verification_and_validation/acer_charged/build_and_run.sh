#!/bin/sh
# Build NJOY2016's own coul/ptlegc/terp1 into a driver and print reference_output.txt
# (gh:#1485). NJOY2016_ROOT = a checkout of NJOY2016 (commit ac5adf5f33d8 for this
# record). acefc.f90 is checked by SHA-256 first; the only change is the `public` line.
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=${NJOY2016_ROOT:?set NJOY2016_ROOT}
want=66ccec86db2fe1acbc9bb8e3713aecc21c9f14255b9ac622eb7eb66ed4f7c3f5
got=$(sha256sum "$root/src/acefc.f90" | cut -d' ' -f1)
[ "$got" = "$want" ] || { echo "acefc.f90 sha256 $got != $want" >&2; exit 1; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT; cd "$tmp"
cp "$root"/src/*.f90 .
sed -i 's/^   public acetop,acefix$/   public acetop,acefix,ptlegc,coul/' acefc.f90
grep -q 'public acetop,acefix,ptlegc,coul' acefc.f90
files=$(ls *.f90 | grep -v '^main.f90$')
for pass in 1 2 3 4 5 6 7 8; do
  left=""
  for f in $files; do o=${f%.f90}.o; [ -f "$o" ] || gfortran -O3 -c "$f" -o "$o" 2>/dev/null || left="$left $f"; done
  files=$left; [ -z "$files" ] && break
done
[ -z "$files" ] || { echo "did not compile: $files" >&2; exit 1; }
cp "$here/driver.f90" "$here/lists.txt" .
gfortran -O3 driver.f90 *.o -o drv
./drv > "$here/reference_output.txt"
echo "wrote $here/reference_output.txt"
