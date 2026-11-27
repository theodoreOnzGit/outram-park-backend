#!/bin/bash
# gh:#1813 round 22: keeps the disk small during a 142-step run that writes every step (10 MB per step).
# usage: reap_fluid_T.sh <transient run dir> <out dir> <START_TIME>
# Every few seconds: for each numeric time directory except the start directory and the two newest, saves
# fluidRegion/T (one line of full doubles) as <out>/T_<k> named by the time directory, T_<time> and deletes the directory.
# Stops when <run dir>/reap.stop exists, after a last pass over everything but the final directory (kept).
R=$1; O=$2; S0=${3:-165}
mkdir -p "$O"
fld() { awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $1}' "$1" | tr '\n' ' '; }
steps() { ls "$R" | grep -E '^[0-9.e+-]+$' | sort -g | grep -v '^0$' | grep -v "^$S0\$"; }
pass() { # $1 = number of newest directories to leave alone
  local dirs n k d
  dirs=$(steps); n=$(echo "$dirs" | grep -c .); k=0
  for d in $dirs; do
    k=$((k+1))
    [ $k -gt $((n-$1)) ] && break
    [ -f "$O/T_$d" ] || { fld "$R/$d/fluidRegion/T" > "$O/T_$d.tmp" && mv "$O/T_$d.tmp" "$O/T_$d"; }
    rm -rf "$R/$d"
  done
}
while [ ! -f "$R/reap.stop" ]; do pass 2; sleep 3; done
pass 1
# the final directory: save its T too, keep the directory
dirs=$(steps); n=$(echo "$dirs" | grep -c .)
last=$(echo "$dirs" | tail -1)
[ -f "$O/T_$last" ] || fld "$R/$last/fluidRegion/T" > "$O/T_$last"
