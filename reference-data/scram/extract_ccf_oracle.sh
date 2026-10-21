#!/bin/bash
# Extract golden oracle values from compiled upstream SCRAM, with COMMON-CAUSE
# FAILURE analysis enabled (`--ccf`).
#
# `extract_oracle.sh` runs without that flag, so on a model with CCF groups it
# records the INDEPENDENT analysis -- a different, and strictly more
# optimistic, question. Both are worth having: this port applies CCF groups by
# default and exposes `MefModel::without_ccf` as the explicit ablation, so both
# fixtures are needed to check both paths.
#
# The only format difference from extract_oracle.sh is that a product member
# may be a <ccf-event>, whose identity is its member list. Upstream's own
# CcfEvent::MakeName joins the member names with a SPACE inside square
# brackets -- "[ValveOne ValveTwo]" -- and this fixture's records are
# whitespace-separated fields, so the separator here is a COMMA instead:
# "[ValveOne,ValveTwo]". That is a property of this file's format, not of the
# name; the consuming test converts the port's own ids the same way, and
# `scram_ccf` says so where it does it.
#
# Output format, one record per model:
#   MODEL <name>
#   TOPNAME <fault tree>
#   EVENT <name> <probability>
#   PRODUCT <order> <event> [<event> ...]
#   TOTAL <exact|rare-event|mcub> <probability>
#   IMPORTANCE <event> <occurrence> <mif> <cif> <dif> <raw> <rrw>
#   END
set -u
SCRAM="$1"; shift

attr() { sed -n "s/.*$2=\"\([^\"]*\)\".*/\1/p" <<< "$1"; }

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  if ! rpt=$("$SCRAM" --probability --importance --ccf "$input" 2>&1) || [ -z "$rpt" ]; then
    echo "skipping $name: scram failed --" >&2
    echo "$rpt" | head -5 >&2
    continue
  fi
  if ! grep -q "<product " <<< "$rpt"; then
    echo "skipping $name: scram produced no products" >&2
    continue
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

  # Products and importance, both of which may name a <ccf-event>.
  awk '
    function attr(line, key,   re) {
      re = key "=\"[^\"]*\""
      if (match(line, re)) return substr(line, RSTART + length(key) + 2, RLENGTH - length(key) - 3)
      return ""
    }
    # A <ccf-event> spans several lines: its attributes open it, its member
    # <basic-event> children follow, and </ccf-event> closes it. Buffer.
    /<ccf-event / {
      inccf = 1; ccfname = ""
      # A self-closing <ccf-event .../> cannot happen: it always has members.
      pending_imp = (index($0, "occurrence=") > 0) ? $0 : ""
      next
    }
    inccf && /<basic-event name=/ {
      ccfname = ccfname (ccfname == "" ? "" : ",") attr($0, "name")
      next
    }
    inccf && /<\/ccf-event>/ {
      inccf = 0
      full = "[" ccfname "]"
      if (pending_imp != "") {
        print "EVENT " full " " attr(pending_imp, "probability")
        imp[++nimp] = "IMPORTANCE " full " " attr(pending_imp, "occurrence") \
                      " " attr(pending_imp, "MIF") " " attr(pending_imp, "CIF") \
                      " " attr(pending_imp, "DIF") " " attr(pending_imp, "RAW") \
                      " " attr(pending_imp, "RRW")
        pending_imp = ""
      } else if (inprod) {
        members = members " " full
      }
      next
    }
    /<product / { order = attr($0, "order"); members = ""; inprod = 1; next }
    # A plain basic event, outside any <ccf-event>.
    inprod && /<basic-event name=/ { members = members " " attr($0, "name"); next }
    inprod && /<\/product>/ { prod[++nprod] = "PRODUCT " order members; inprod = 0; next }
    /<basic-event name=.*probability=/ {
      print "EVENT " attr($0, "name") " " attr($0, "probability")
      if (index($0, "MIF=") > 0) {
        imp[++nimp] = "IMPORTANCE " attr($0, "name") " " attr($0, "occurrence") \
                      " " attr($0, "MIF") " " attr($0, "CIF") " " attr($0, "DIF") \
                      " " attr($0, "RAW") " " attr($0, "RRW")
      }
      next
    }
    END {
      for (i = 1; i <= nprod; i++) print prod[i]
      for (i = 1; i <= nimp; i++) print imp[i]
    }
  ' <<< "$rpt"

  for mode in exact rare-event mcub; do
    case $mode in
      exact) flag="" ;;
      rare-event) flag="--rare-event" ;;
      mcub) flag="--mcub" ;;
    esac
    tot=$("$SCRAM" --probability --ccf $flag "$input" 2>/dev/null \
          | grep -oE '<sum-of-products[^>]*probability="[0-9.eE+-]+"' | head -1 \
          | sed -n 's/.*probability="\([^"]*\)".*/\1/p')
    [ -n "$tot" ] && echo "TOTAL $mode $tot"
  done

  echo "END"
done
