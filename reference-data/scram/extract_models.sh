#!/bin/bash
# Extract fault-tree STRUCTURE from upstream SCRAM's own input models.
#
# This is deliberately a SEPARATE script from extract_oracle.sh, and the split
# is the point. `extract_oracle.sh` parses SCRAM's own XML *report*, so nothing
# it emits can drift from what SCRAM computed. This script parses SCRAM's
# *input* models, which is the opposite direction — but it only ever emits the
# tree structure, never an answer. Probabilities and cut sets stay in
# oracle.txt, where they come from SCRAM.
#
# So the comparison is: feed this structure to raffles::scram::mocus, and check
# the cut sets it generates against the ones SCRAM reported for the same model.
#
# Only the flat single-connective gate form used by these models is handled:
# one <and>/<or>/<atleast>/<null> per <define-gate>, or a bare <event/> child
# meaning a pass-through (NULL) gate. A nested formula would be silently
# mis-parsed, so the script REFUSES a gate it cannot read rather than guessing
# — see the "cannot parse" check below.
#
# Output format, one record per line:
#   MODEL <name>
#   GATE  <name> <connective> <min-or-dash> <g:arg|b:arg> ...
#   HOUSE <house-event> <true|false>
#   PARAM <basic-event> <probability>
#   TOP   <name>
#   END
#
# PARAM is emitted ONLY for a basic event declared with a direct <float>, and
# it is the one number this script reports. It is still part of the question,
# not an answer: it is what the model says, not what SCRAM computed. It exists
# because a BDD evaluates the whole Boolean function and so needs EVERY basic
# event's probability, while SCRAM's report prices only the events that
# survive into some product. `Lift`'s W_1 and the small non-coherent model's
# `b` are exactly that case. An event defined by an expression (GLM,
# periodic-test, ...) gets no PARAM, because this script does not evaluate
# expressions -- for those the oracle's own value is the only source.
set -u

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  awk -v model="$name" '
    # --- strip XML comments, including multi-line ones -------------------
    {
      line = $0
      while (1) {
        if (incomment) {
          e = index(line, "-->")
          if (e == 0) { line = ""; break }
          line = substr(line, e + 3); incomment = 0
        } else {
          s = index(line, "<!--")
          if (s == 0) break
          rest = substr(line, s + 4)
          e = index(rest, "-->")
          if (e == 0) { line = substr(line, 1, s - 1); incomment = 1; break }
          line = substr(line, 1, s - 1) substr(rest, e + 3)
        }
      }
      $0 = line
      if ($0 ~ /^[[:space:]]*$/) next
    }

    function attr(s, key,   re, m) {
      re = key "=\"[^\"]*\""
      if (match(s, re)) { m = substr(s, RSTART, RLENGTH); sub(key "=\"", "", m); sub(/"$/, "", m); return m }
      return ""
    }

    # --- pass 1 is folded in: record gate names as we go -----------------
    /<define-gate[ >]/ {
      gname = attr($0, "name")
      ngates++; order[ngates] = gname; isgate[gname] = 1
      conn[gname] = ""; min[gname] = "-"; args[gname] = ""
      ingate = 1; next
    }
    /<\/define-gate>/ { ingate = 0; next }

    # --- house events: constants, not probabilities -----------------------
    /<define-house-event[ >]/ {
      hname = attr($0, "name"); inhe = 1
      if ($0 ~ /<constant[ ]/) house[hname] = attr($0, "value")
      if ($0 ~ /<\/define-house-event>/) inhe = 0
      next
    }
    /<\/define-house-event>/ { inhe = 0; next }
    inhe && /<constant[ ]/ { house[hname] = attr($0, "value"); next }
    inhe { next }

    # --- basic-event probabilities, direct <float> only -------------------
    /<define-basic-event[ >]/ {
      bname = attr($0, "name"); inbe = 1; bekind = ""
      # The whole declaration may sit on one line, in which case the float and
      # the closing tag are right here and the per-line rules below never see
      # them.
      if ($0 ~ /<float[ ]/) { bekind = "float"; beval = attr($0, "value") }
      if ($0 ~ /<\/define-basic-event>/) {
        if (bekind == "float") param[bname] = beval
        inbe = 0
      }
      next
    }
    /<\/define-basic-event>/ {
      if (inbe && bekind == "float") param[bname] = beval
      inbe = 0; next
    }
    inbe && /<label[ >]/ { next }
    inbe && /<attributes[ >]/ { next }
    inbe && /<attribute[ >]/ { next }
    inbe && /<\/attributes>/ { next }
    inbe && bekind == "" && /<float[ ]/ { bekind = "float"; beval = attr($0, "value"); next }
    # Anything else that opens an element is an expression this script does
    # not evaluate; the event is left without a PARAM rather than guessed at.
    inbe && bekind == "" && /</ { bekind = "expression"; next }

    ingate && /<(and|or|atleast|xor|not|nand|nor|null)[ >]/ {
      if (match($0, /<(and|or|atleast|xor|not|nand|nor|null)[ >]/)) {
        c = substr($0, RSTART + 1, RLENGTH - 2)
        sub(/[ >]$/, "", c)
        if (conn[gname] != "") { bad[gname] = "nested or repeated connective" }
        conn[gname] = c
        if (c == "atleast") { m = attr($0, "min"); if (m == "") bad[gname] = "atleast without min"; else min[gname] = m }
      }
      next
    }

    ingate && /<(event|basic-event|gate|house-event)[ ]/ {
      an = attr($0, "name")
      if (an == "") next
      args[gname] = args[gname] " " an
      if ($0 ~ /<gate[ ]/) forced[gname SUBSEP an] = "g"
      if ($0 ~ /<basic-event[ ]/) forced[gname SUBSEP an] = "b"
      if ($0 ~ /<house-event[ ]/) forced[gname SUBSEP an] = "h"
      next
    }

    # A model assembled by XInclude is only partly in this file; emitting the
    # fragment we can see would be a half tree that looks whole.
    /<xi:include/ { included = 1 }

    # <define-component> opens a PRIVATE NAMESPACE, so a gate inside it named
    # `E1` is really `t.E1` and is a different gate from an outer `E1`. This
    # script has no notion of namespaces and would silently merge the two --
    # which is exactly what it did to ThreeMotor before this guard.
    /<define-component/ { componented = 1 }

    END {
      print "MODEL " model
      if (included) { print "CANNOT-PARSE <model> assembled by xi:include, structure is incomplete" }
      if (componented) { print "CANNOT-PARSE <model> uses define-component, whose private namespaces this script does not model" }
      for (i = 1; i <= ngates; i++) {
        g = order[i]
        c = conn[g]
        n = split(args[g], a, " ")
        if (c == "") {
          if (n == 1) c = "null"; else bad[g] = "no connective and " n " arguments"
        }
        if (g in bad) { print "CANNOT-PARSE " g " " bad[g]; continue }
        rec = "GATE " g " " c " " min[g]
        for (j = 1; j <= n; j++) {
          kind = forced[g SUBSEP a[j]]
          if (kind == "") kind = (a[j] in isgate) ? "g" : ((a[j] in house) ? "h" : "b")
          rec = rec " " kind ":" a[j]
          if (kind == "g") referenced[a[j]] = 1
        }
        print rec
      }
      for (hn in house) print "HOUSE " hn " " house[hn]
      for (bn in param) print "PARAM " bn " " param[bn]
      # The top gate is the one no other gate names as an argument.
      ntop = 0
      for (i = 1; i <= ngates; i++) if (!(order[i] in referenced)) { ntop++; top = order[i] }
      if (ntop == 1) print "TOP " top
      else print "CANNOT-PARSE <top> " ntop " candidate top gates"
      print "END"
    }
  ' "$input"
done
