#!/bin/bash
# Extract golden oracle values from compiled upstream SCRAM for models with
# SUBSTITUTIONS.
#
# These need their own generator for one reason: SCRAM REFUSES AN EXACT
# ANALYSIS of a model with non-declarative substitutions --
#
#     scram::mef::ValidityError
#     Non-declarative substitutions do not apply to exact analyses.
#
# -- because the rewritten product list is no longer the minimal cut sets of
# any Boolean function, so inclusion-exclusion over it means nothing. The
# products themselves are still reported under an approximation. So this runs
# the unflagged (exact) analysis first and falls back to `--rare-event` for the
# report, recording whichever totals the binary will produce and no others.
# A model whose totals are missing from a record is a model SCRAM refused to
# compute them for, which is information rather than an omission.
#
# Output format is extract_oracle.sh's, one record per model.
set -u
SCRAM="$1"; shift

attr() { sed -n "s/.*$2=\"\([^\"]*\)\".*/\1/p" <<< "$1"; }

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  mode_flag=""
  if ! rpt=$("$SCRAM" --probability --importance "$input" 2>&1) \
     || ! grep -q "<product " <<< "$rpt"; then
    mode_flag="--rare-event"
    if ! rpt=$("$SCRAM" --probability --importance $mode_flag "$input" 2>&1) \
       || ! grep -q "<product " <<< "$rpt"; then
      echo "skipping $name: scram failed under both exact and --rare-event --" >&2
      echo "$rpt" | head -5 >&2
      continue
    fi
    echo "note: $name needs an approximation; SCRAM refuses an exact analysis" >&2
  fi

  nresults=$(grep -c "<sum-of-products " <<< "$rpt")
  if [ "$nresults" -ne 1 ]; then
    echo "skipping $name: $nresults result sets" >&2
    continue
  fi

  echo "MODEL $name"
  analysed=$(grep -oE '<sum-of-products name="[^"]*"' <<< "$rpt" | head -1 \
             | sed -n 's/.*name="\([^"]*\)".*/\1/p')
  [ -n "$analysed" ] && echo "TOPNAME $analysed"

  while IFS= read -r line; do
    ev=$(attr "$line" name); p=$(attr "$line" probability)
    [ -n "$ev" ] && [ -n "$p" ] && echo "EVENT $ev $p"
  done < <(grep "<basic-event name=.*probability=" <<< "$rpt")

  awk '
    /<product /  { order=""; if (match($0, /order="[0-9]+"/)) { order=substr($0, RSTART+7, RLENGTH-8) }
                   members=""; inprod=1; next }
    inprod && /<basic-event name=/ {
        if (match($0, /name="[^"]*"/)) { members = members " " substr($0, RSTART+6, RLENGTH-7) } next }
    inprod && /<\/product>/ { print "PRODUCT " order members; inprod=0; next }
  ' <<< "$rpt"

  for mode in exact rare-event mcub; do
    case $mode in
      exact) flag="" ;;
      rare-event) flag="--rare-event" ;;
      mcub) flag="--mcub" ;;
    esac
    out=$("$SCRAM" --probability $flag "$input" 2>/dev/null) || continue
    tot=$(grep -oE '<sum-of-products[^>]*probability="[0-9.eE+-]+"' <<< "$out" | head -1 \
          | sed -n 's/.*probability="\([^"]*\)".*/\1/p')
    [ -n "$tot" ] && echo "TOTAL $mode $tot"
  done

  while IFS= read -r line; do
    ev=$(attr "$line" name)
    echo "IMPORTANCE $ev $(attr "$line" occurrence) $(attr "$line" MIF) $(attr "$line" CIF) $(attr "$line" DIF) $(attr "$line" RAW) $(attr "$line" RRW)"
  done < <(grep "<basic-event name=.*MIF=" <<< "$rpt")

  echo "END"
done
