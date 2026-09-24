#!/bin/bash
# Extract PRIME IMPLICANTS from compiled upstream SCRAM (`--prime-implicants`).
#
# A third fixture beside oracle.txt and oracle-noncoherent.txt, and separate
# for the same reason they are: it answers a different question. A minimal cut
# set is a set of components whose failure suffices; a prime implicant also
# records which components must be WORKING, so it carries negated literals and
# describes the function exactly rather than conservatively.
#
# For a COHERENT tree the two coincide, which the tests use as an invariant.
#
# Like extract_oracle.sh, everything here comes from SCRAM's own XML report.
#
# Output format, one record per line:
#   MODEL <name>
#   EVENT <name> <probability>
#   PI    <order> <+event|-event> ...
#   TOTAL exact <probability>
#   END
set -u
SCRAM="$1"; shift

attr() { sed -n "s/.*$2=\"\([^\"]*\)\".*/\1/p" <<< "$1"; }

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  if ! rpt=$(timeout 300 "$SCRAM" --probability --importance --prime-implicants "$input" 2>&1) \
     || [ -z "$rpt" ]; then
    echo "skipping $name: scram failed or timed out under --prime-implicants" >&2
    continue
  fi
  nresults=$(grep -c "<sum-of-products " <<< "$rpt")
  if [ "$nresults" -ne 1 ]; then
    echo "skipping $name: $nresults result sets" >&2
    continue
  fi
  grep -q "<product " <<< "$rpt" || { echo "skipping $name: no products" >&2; continue; }

  echo "MODEL $name"

  while IFS= read -r line; do
    ev=$(attr "$line" name); p=$(attr "$line" probability)
    [ -n "$ev" ] && [ -n "$p" ] && echo "EVENT $ev $p"
  done < <(grep "<basic-event name=.*probability=" <<< "$rpt")

  # Products, with the sign of each literal. A <not> wrapper negates the
  # <basic-event> that follows it.
  awk '
    /<product /  { order=""; if (match($0, /order="[0-9]+"/)) { order=substr($0, RSTART+7, RLENGTH-8) }
                   members=""; inprod=1; innot=0; next }
    inprod && /<not>/        { innot=1; next }
    inprod && /<\/not>/      { innot=0; next }
    inprod && /<basic-event name=/ {
        if (match($0, /name="[^"]*"/)) {
            nm = substr($0, RSTART+6, RLENGTH-7)
            members = members " " (innot ? "-" : "+") nm
        }
        next }
    inprod && /<\/product>/ { print "PI " order members; inprod=0; next }
  ' <<< "$rpt"

  tot=$(grep -oE '<sum-of-products[^>]*probability="[0-9.eE+-]+"' <<< "$rpt" | head -1 \
        | sed -n 's/.*probability="\([^"]*\)".*/\1/p')
  [ -n "$tot" ] && echo "TOTAL exact $tot"
  echo "END"
done
