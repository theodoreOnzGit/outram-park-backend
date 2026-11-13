#!/bin/bash
# Runs every inputs/*.xml through a CYCLUS binary and dumps the Transactions
# table to cyclus_output/<case>.csv with columns
#   time,sender_proto,receiver_proto,commodity,quantity
# quantity is printed with 17 significant digits so binary64 round-trips.
# Usage: run_cyclus.sh <conda-env-prefix>
set -eu
E=$1
cd "$(dirname "$0")"
export PATH="$E/bin:$PATH" LD_LIBRARY_PATH="$E/lib" CYCLUS_PATH="$E/lib/cyclus" CYCLUS_NUC_DATA="$E/share/cyclus/cyclus_nuc_data.h5"
mkdir -p cyclus_output
for f in inputs/*.xml; do
  c=$(basename "$f" .xml)
  rm -f "/tmp/$c.sqlite"
  cyclus -o "/tmp/$c.sqlite" "$f" > /dev/null 2>&1
  sqlite3 -csv "/tmp/$c.sqlite" \
    "SELECT t.Time, s.Prototype, r.Prototype, t.Commodity, printf('%.17g', res.Quantity)
     FROM Transactions t
     JOIN AgentEntry s ON s.AgentId=t.SenderId
     JOIN AgentEntry r ON r.AgentId=t.ReceiverId
     JOIN Resources res ON res.ResourceId=t.ResourceId
     ORDER BY t.Time, t.TransactionId" > "cyclus_output/$c.csv"
  echo "$c: $(wc -l < cyclus_output/$c.csv) transactions"
done
