#!/bin/bash
# usage: run.sh <binary> <firstseed> <outfile>
ulimit -v 12582912
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf
cd /tmp
"$1" --purr --particles 4000 --inactive 40 --active 100 --seeds 8 --seed "$2" --threads 2 > "$3" 2>&1
echo EXIT $? >> "$3"
