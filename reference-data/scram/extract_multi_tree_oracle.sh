#!/bin/bash
# Extract golden oracle values from compiled upstream SCRAM, for models that
# define SEVERAL fault trees.
#
# `extract_oracle.sh` refuses those models outright: its record format has one
# top event per model, and a report with several <sum-of-products> gives no way
# to say which product belongs to which tree, so merging them would be a
# silently wrong fixture. SCRAM does name each result set after the fault tree
# it analysed, though, so the answer is not lost -- it just needs a record per
# TREE rather than per MODEL. That is what this emits.
#
# Everything here comes from SCRAM's own XML report. Nothing is parsed out of
# the input model.
#
# Output format, one record per fault tree:
#   MODEL <name>
#   TREE <fault tree name>
#   EVENT <name> <probability>
#   PRODUCT <order> <event> [<event> ...]
#   TOTAL <exact|rare-event|mcub> <probability>
#   IMPORTANCE <event> <occurrence> <mif> <cif> <dif> <raw> <rrw>
#   END
set -u
SCRAM="$1"; shift

# Per-tree totals for one approximation mode: "<tree> <probability>" lines.
totals() {
  "$SCRAM" --probability $2 "$1" 2>/dev/null \
    | grep -oE '<sum-of-products name="[^"]*"[^>]*probability="[0-9.eE+-]+"' \
    | sed -n 's/.*name="\([^"]*\)".*probability="\([^"]*\)".*/\1 \2/p'
}

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  if ! rpt=$("$SCRAM" --probability --importance "$input" 2>&1) || [ -z "$rpt" ]; then
    echo "skipping $name: scram failed --" >&2
    echo "$rpt" | head -5 >&2
    continue
  fi
  if ! grep -q "<product " <<< "$rpt"; then
    echo "skipping $name: scram produced no products" >&2
    continue
  fi

  exact=$(totals "$input" "")
  rare=$(totals "$input" "--rare-event")
  mcub=$(totals "$input" "--mcub")

  awk -v model="$name" \
      -v exact="$exact" -v rare="$rare" -v mcub="$mcub" '
    function attr(line, key,   re) {
      re = key "=\"[^\"]*\""
      if (match(line, re)) return substr(line, RSTART + length(key) + 2, RLENGTH - length(key) - 3)
      return ""
    }
    function load(text, mode,   n, i, parts, fields) {
      n = split(text, parts, "\n")
      for (i = 1; i <= n; i++) {
        if (split(parts[i], fields, " ") == 2) total[fields[1] SUBSEP mode] = fields[2]
      }
    }
    BEGIN { load(exact, "exact"); load(rare, "rare-event"); load(mcub, "mcub") }

    /<sum-of-products / {
      tree = attr($0, "name")
      print "MODEL " model
      print "TREE " tree
      # The importance block for this tree follows the products, but EVENT
      # lines read better first, so they are buffered and flushed at END.
      nprod = 0
      insop = 1
      next
    }
    insop && /<product / { order = attr($0, "order"); members = ""; inprod = 1; next }
    inprod && /<basic-event name=/ { members = members " " attr($0, "name"); next }
    inprod && /<\/product>/ { prod[++nprod] = "PRODUCT " order members; inprod = 0; next }
    insop && /<\/sum-of-products>/ { insop = 0; next }

    /<importance name=/ { inimp = 1; nev = 0; nimp = 0; next }
    inimp && /<basic-event name=/ {
      ev = attr($0, "name")
      print "EVENT " ev " " attr($0, "probability")
      imp[++nimp] = "IMPORTANCE " ev " " attr($0, "occurrence") " " attr($0, "MIF") \
                    " " attr($0, "CIF") " " attr($0, "DIF") " " attr($0, "RAW") \
                    " " attr($0, "RRW")
      next
    }
    inimp && /<\/importance>/ {
      for (i = 1; i <= nprod; i++) print prod[i]
      for (m = 1; m <= 3; m++) {
        mode = (m == 1 ? "exact" : (m == 2 ? "rare-event" : "mcub"))
        if ((tree SUBSEP mode) in total) print "TOTAL " mode " " total[tree SUBSEP mode]
      }
      for (i = 1; i <= nimp; i++) print imp[i]
      print "END"
      inimp = 0
      next
    }
  ' <<< "$rpt"
done
