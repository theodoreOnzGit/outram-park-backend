#!/bin/bash
# Extract golden oracle values from compiled upstream SCRAM.
#
# Everything emitted here comes from SCRAM's own XML report — the basic-event
# probabilities from the <importance> section, the cut sets from <product>
# elements, and the three totals from three separate runs. Nothing is parsed
# out of the input model, so the fixture cannot drift from what SCRAM actually
# computed.
#
# Output format, one record per line:
#   MODEL <name>
#   EVENT <name> <probability>
#   PRODUCT <order> <event> [<event> ...]
#   TOTAL <exact|rare-event|mcub> <probability>
#   IMPORTANCE <event> <occurrence> <mif> <cif> <dif> <raw> <rrw>
set -u
SCRAM="$1"; shift

attr() { sed -n "s/.*$2=\"\([^\"]*\)\".*/\1/p" <<< "$1"; }

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  rpt=$("$SCRAM" --probability --importance "$input" 2>/dev/null) || continue
  [ -z "$rpt" ] && continue
  grep -q "<product " <<< "$rpt" || continue

  echo "MODEL $name"

  # Basic-event probabilities, from the importance section.
  while IFS= read -r line; do
    ev=$(attr "$line" name); p=$(attr "$line" probability)
    [ -n "$ev" ] && [ -n "$p" ] && echo "EVENT $ev $p"
  done < <(grep "<basic-event name=.*probability=" <<< "$rpt")

  # Cut sets: a <product> opens, its <basic-event> children follow.
  awk '
    /<product /  { order=""; if (match($0, /order="[0-9]+"/)) { order=substr($0, RSTART+7, RLENGTH-8) }
                   members=""; inprod=1; next }
    inprod && /<basic-event name=/ {
        if (match($0, /name="[^"]*"/)) { members = members " " substr($0, RSTART+6, RLENGTH-7) } next }
    inprod && /<\/product>/ { print "PRODUCT " order members; inprod=0; next }
  ' <<< "$rpt"

  # Totals, three runs.
  for mode in exact rare-event mcub; do
    case $mode in
      exact) flag="" ;;
      rare-event) flag="--rare-event" ;;
      mcub) flag="--mcub" ;;
    esac
    tot=$("$SCRAM" --probability $flag "$input" 2>/dev/null \
          | grep -oE '<sum-of-products[^>]*probability="[0-9.eE+-]+"' | head -1 \
          | sed -n 's/.*probability="\([^"]*\)".*/\1/p')
    [ -z "$tot" ] && tot=$("$SCRAM" --probability $flag "$input" 2>/dev/null \
          | grep -oE 'probability="[0-9.eE+-]+"' | tail -1 \
          | sed -n 's/probability="\([^"]*\)"/\1/p')
    [ -n "$tot" ] && echo "TOTAL $mode $tot"
  done

  # Importance factors.
  while IFS= read -r line; do
    ev=$(attr "$line" name)
    echo "IMPORTANCE $ev $(attr "$line" occurrence) $(attr "$line" MIF) $(attr "$line" CIF) $(attr "$line" DIF) $(attr "$line" RAW) $(attr "$line" RRW)"
  done < <(grep "<basic-event name=.*MIF=" <<< "$rpt")

  echo "END"
done
