#!/bin/bash
# Extract golden oracle values from compiled upstream SCRAM for models with
# ALIGNMENTS.
#
# A model with an alignment has no single answer: upstream analyses it once per
# phase and tags each result set with `alignment=` and `phase=`. This emits one
# record per (alignment, phase), which is the shape extract_oracle.sh's
# one-result-per-model format cannot hold.
#
# It is run TWICE per model, with and without `--ccf`, and each record says
# which. `TwoTrain/two_train_alignment.xml` declares common-cause groups as
# well as phases, and this port applies CCF groups by default where upstream
# needs the flag -- so both runs are needed to check both of this port's paths
# against the upstream run that answers the same question.
#
# Output format, one record per (alignment, phase, ccf setting):
#   MODEL <name>
#   CCF <on|off>
#   ALIGNMENT <name>
#   PHASE <name>
#   TOPNAME <fault tree>
#   EVENT <name> <probability>
#   PRODUCT <order> <event> [<event> ...]
#   TOTAL exact <probability>
#   END
#
# A <ccf-event> member is rendered as its comma-separated member list inside
# square brackets, as in extract_ccf_oracle.sh; upstream's own name uses a
# space, and the comma is this format's, since the records are
# whitespace-separated.
set -u
SCRAM="$1"; shift

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  for ccf in off on; do
    flag=""
    [ "$ccf" = "on" ] && flag="--ccf"
    if ! rpt=$("$SCRAM" --probability $flag "$input" 2>&1) \
       || ! grep -q "<product " <<< "$rpt"; then
      echo "skipping $name (ccf $ccf): scram produced no products --" >&2
      echo "$rpt" | head -5 >&2
      continue
    fi

    awk -v model="$name" -v ccf="$ccf" '
      function attr(line, key,   re) {
        re = key "=\"[^\"]*\""
        if (match(line, re)) return substr(line, RSTART + length(key) + 2, RLENGTH - length(key) - 3)
        return ""
      }
      /<sum-of-products / {
        if (open) print "END"
        print "MODEL " model
        print "CCF " ccf
        print "ALIGNMENT " attr($0, "alignment")
        print "PHASE " attr($0, "phase")
        print "TOPNAME " attr($0, "name")
        print "TOTAL exact " attr($0, "probability")
        open = 1
        next
      }
      /<ccf-event / { inccf = 1; ccfname = ""; next }
      inccf && /<basic-event name=/ {
        ccfname = ccfname (ccfname == "" ? "" : ",") attr($0, "name")
        next
      }
      inccf && /<\/ccf-event>/ { inccf = 0; members = members " [" ccfname "]"; next }
      /<product / { order = attr($0, "order"); members = ""; inprod = 1; next }
      inprod && /<basic-event name=/ { members = members " " attr($0, "name"); next }
      inprod && /<\/product>/ { print "PRODUCT " order members; inprod = 0; next }
      END { if (open) print "END" }
    ' <<< "$rpt"
  done
done
