#!/bin/sh
# Rebuild elastic_recoil_reference.txt (gh:#1490): NJOY2016 with
# elastic_recoil_instrument.diff (writes in acelcp's mt.eq.2 block,
# acefc.f90:9555-9636), every file at gfortran -O3, run on test deck 54
# (p + H-3); the ER* lines of its listing are compacted into
#   HEAD land and landh+jp-1 andh hpd it nes esz next xss(land)
#   AWR  awr awi                      (IEEE-754 bits)
#   UBAR ie e ubar heat               (bits; diagnosis only)
#   <IN|ESZ|HPD|OUT|HOUT> start n     then n words as bits, 8 per line
# and unit 31 is checked byte-identical to the unpatched NJOY2016's.
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=${NJOY2016_ROOT:?set NJOY2016_ROOT}
want=66ccec86db2fe1acbc9bb8e3713aecc21c9f14255b9ac622eb7eb66ed4f7c3f5
got=$(sha256sum "$root/src/acefc.f90" | cut -d' ' -f1)
[ "$got" = "$want" ] || { echo "acefc.f90 sha256 $got != $want" >&2; exit 1; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT; cd "$tmp"
mkdir src run ref; cd src
cp "$root"/src/*.f90 .
patch -p1 < "$here/elastic_recoil_instrument.diff"
files=$(ls *.f90)
for pass in 1 2 3 4 5 6 7 8; do
  left=""
  for f in $files; do o=${f%.f90}.o; [ -f "$o" ] || gfortran -O3 -c "$f" -o "$o" 2>/dev/null || left="$left $f"; done
  files=$left; [ -z "$files" ] && break
done
[ -z "$files" ] || { echo "did not compile: $files" >&2; exit 1; }
gfortran -O3 *.o -o njoy
for d in run ref; do cp "$root/tests/resources/p-001_H_003-ENDF8.0.endf" "../$d/tape20"; done
(cd ../run && ../src/njoy < "$root/tests/54/input" > output 2>&1) || true
(cd ../ref && "$root/build/njoy" < "$root/tests/54/input" > output 2>&1) || true
cmp ../run/tape31 ../ref/tape31
awk '
  /^ERHEAD/ { $1 = "HEAD"; print; next }
  /^ERAWR/  { $1 = "AWR"; print; next }
  /^ERUBAR/ { $1 = "UBAR"; print; next }
  /^ER/ {
    name = substr($1, 3)
    if (!(name in start)) { start[name] = $2; order[++n] = name }
    w[name, ++cnt[name]] = $3
  }
  END {
    for (k = 1; k <= n; k++) {
      name = order[k]; print name, start[name], cnt[name]
      line = ""
      for (i = 1; i <= cnt[name]; i++) {
        line = (line == "" ? w[name, i] : line " " w[name, i])
        if (i % 8 == 0 || i == cnt[name]) { print line; line = "" }
      }
    }
  }' ../run/output > "$here/elastic_recoil_reference.txt"
echo "wrote $here/elastic_recoil_reference.txt"
