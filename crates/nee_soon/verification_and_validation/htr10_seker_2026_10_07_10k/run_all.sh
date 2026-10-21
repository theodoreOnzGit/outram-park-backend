#!/bin/bash
# The exact launch of this record: the shared launcher at its defaults
# (10 000 x [5 + 135], N = 10..20, VIII.0 and VII.0, 5 slots x 7 threads).
cd "$(dirname "$0")" && exec ../htr10_run_all.sh .
