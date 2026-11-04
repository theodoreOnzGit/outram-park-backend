#!/usr/bin/env bash
# gh:#1192: regenerate the OpenDA reference outputs in ./references.
#
# Builds OpenDA's core, models, observers and algorithms Java sources (no Ant,
# no Maven: plain javac against the jars OpenDA vendors), then compiles and runs
# driver/org/openda/algorithms/kalmanFilter/Gh1192Driver.java.
#
#   usage: generate_references.sh <scratch-dir>
#
# Recorded run (2026-11-04): OpenJDK 21.0.12.1 (Ubuntu 24.04), x86_64,
# OpenDA commit e51565c2c8def7d35f8b378a9ac118edbd98a94d (2026-10-23).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRATCH="${1:?scratch dir}"
ODA="$SCRATCH/upstream/openda"
if [ ! -d "$ODA" ]; then
  git clone https://github.com/OpenDA-Association/OpenDA.git "$ODA"
  git -C "$ODA" checkout e51565c2c8def7d35f8b378a9ac118edbd98a94d
fi
BUILD="$SCRATCH/odabuild"
mkdir -p "$BUILD"
CP=$(find "$ODA/core/java/resources" "$ODA/observers/java/resources" -name "*.jar" | tr '\n' ':')
if [ ! -f "$BUILD/org/openda/algorithms/kalmanFilter/EnKF.class" ]; then
  find "$ODA/core/java/src" "$ODA/models/java/src" "$ODA/algorithms/java/src" "$ODA/observers/java/src" \
    -name "*.java" > "$SCRATCH/srcs.txt"
  javac -nowarn -encoding UTF-8 -d "$BUILD" -cp "$CP" @"$SCRATCH/srcs.txt"
fi
javac -nowarn -encoding UTF-8 -d "$BUILD" -cp "$BUILD:$CP" \
  "$HERE/driver/org/openda/algorithms/kalmanFilter/Gh1192Driver.java"
# OpenDA's StochVector prints from its static initialiser; the driver replaces its generator.
java -Xmx1g -Dgh1192.fixtures="$HERE/fixtures" -cp "$BUILD:$CP" \
  org.openda.algorithms.kalmanFilter.Gh1192Driver "$SCRATCH/gh1192_out" > "$SCRATCH/gh1192_driver.log" 2>&1
cp -f "$SCRATCH/gh1192_out/openda_sequential.json" "$SCRATCH/gh1192_out/openda_calibration.json" \
  "$SCRATCH/gh1192_out/openda_linalg.json" "$HERE/references/"
tail -1 "$SCRATCH/gh1192_driver.log"
