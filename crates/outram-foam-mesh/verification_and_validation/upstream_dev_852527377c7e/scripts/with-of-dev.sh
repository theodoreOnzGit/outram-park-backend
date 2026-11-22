#!/bin/bash
# Run "$@" inside the OpenFOAM-dev (dummy Pstream) environment.
source /home/user/upstream-build/scripts/of-dev-env.sh >/dev/null 2>&1
exec "$@"
