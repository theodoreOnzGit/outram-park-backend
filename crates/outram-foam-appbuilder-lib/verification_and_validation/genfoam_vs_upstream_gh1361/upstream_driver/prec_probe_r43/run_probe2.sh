#!/bin/bash
# round 43 probe on the TIGHT t=165 state (the state the committed reference msfr_transient.ref starts from)
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export LD_LIBRARY_PATH=/home/user/upstream-build/gh1361/r43/lib:$LD_LIBRARY_PATH
W=/home/user/upstream-build/gh1361/r43/run2
rm -rf $W; mkdir -p $W
S=/home/user/upstream-build/gh1361/r43/chain/transient
cp -r $S/constant $S/system $S/165 $W/
cd $W || exit 1
sed -i 's/^endTime .*/endTime 165.0031;/; s/^writeInterval .*/writeInterval   1000;/' system/controlDict
GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
ls | head -5
