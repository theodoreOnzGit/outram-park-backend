#!/bin/bash
# Extract golden UNCERTAINTY values from compiled upstream SCRAM.
#
# `scram --uncertainty` runs a Monte Carlo over the model's random deviates and
# reports the distribution of the top-event probability. The statistics are the
# oracle; the individual samples cannot be, because upstream draws from one
# static std::mt19937 and this port from outram_mc_libs::rng::lcg. Two Monte
# Carlo runs from different streams agree in distribution and never sample for
# sample, which is why the consuming test is statistical and says at what
# confidence.
#
# The <histogram> is deliberately NOT extracted. Upstream's bin edges come from
# boost::accumulators' density accumulator, whose range is its own: on
# SmallTree its first edge is 0.0021956 with width 0.0100408, which is not
# (max - min) / 20 and is not documented as anything reproducible. Recording
# numbers that cannot be reproduced would read as a comparison that was never
# made.
#
# Output format, one record per model:
#   MODEL <name>
#   TRIALS <n>
#   MEAN <value>
#   SIGMA <value>
#   ERROR-FACTOR <value>
#   CONFIDENCE <lower> <upper>
#   QUANTILE <number> <probability> <value>
#   END
set -u
SCRAM="$1"; shift

attr() { sed -n "s/.*$2=\"\([^\"]*\)\".*/\1/p" <<< "$1"; }

for input in "$@"; do
  name=$(basename "$(dirname "$input")")/$(basename "$input" .xml)
  if ! rpt=$("$SCRAM" --uncertainty --probability "$input" 2>&1) \
     || ! grep -q "<measure " <<< "$rpt"; then
    echo "skipping $name: scram produced no uncertainty measure --" >&2
    echo "$rpt" | head -5 >&2
    continue
  fi

  echo "MODEL $name"
  # SCRAM's own default, Settings::num_trials_ = 1e3. Recorded because every
  # number below is a Monte Carlo estimate and its error depends on it.
  echo "TRIALS 1000"
  echo "MEAN $(attr "$(grep -m1 '<mean ' <<< "$rpt")" value)"
  echo "SIGMA $(attr "$(grep -m1 '<standard-deviation ' <<< "$rpt")" value)"
  echo "ERROR-FACTOR $(attr "$(grep -m1 '<error-factor ' <<< "$rpt")" value)"
  ci=$(grep -m1 '<confidence-range ' <<< "$rpt")
  echo "CONFIDENCE $(attr "$ci" lower-bound) $(attr "$ci" upper-bound)"

  # Each <quantile> reports the band it closes; its upper bound is the
  # quantile value at that probability.
  while IFS= read -r line; do
    echo "QUANTILE $(attr "$line" number) $(attr "$line" value) $(attr "$line" upper-bound)"
  done < <(grep '<quantile ' <<< "$rpt")

  echo "END"
done
