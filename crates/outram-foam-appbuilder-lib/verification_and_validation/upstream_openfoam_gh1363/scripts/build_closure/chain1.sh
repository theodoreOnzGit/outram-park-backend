#!/bin/bash
cd /home/user/upstream-build/gh1363/closure
for d in thermophysicalModels/specie thermophysicalModels/thermophysicalProperties transportModels/compressible thermophysicalModels/basic; do
  ./build_lib.sh $d || break
done
