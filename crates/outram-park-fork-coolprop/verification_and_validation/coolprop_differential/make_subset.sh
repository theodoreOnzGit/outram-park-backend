#!/bin/sh
# Deterministic subset of the full CoolProp reference, small enough to commit.
#   usage: make_subset.sh full.csv > subset.csv
# Rules (fixed in advance, not tuned): META kept; E rows with idx % 9 == 0 plus idx 156
# (the exact critical point); per fluid and tag, SATA / SAT / SATS rows k = 0..29 with
# k % 5 == 0 or k == 29; PT rows k = 0..99 with k % 8 == 0.
awk -F, '
$1 == "META" { print; next }
$1 == "E"    { if (($3 % 9) == 0 || $3 == 156) print; next }
$1 == "SATA" || $1 == "SAT" || $1 == "SATS" {
    key = $1 SUBSEP $2; k = c[key]++;
    if ((k % 5) == 0 || k == 29) print;
    next
}
$1 == "PT"   { key = "PT" SUBSEP $2; k = c[key]++; if ((k % 8) == 0) print; next }
' "$1"
