#!/bin/bash
# Writes the CYCLUS input files for the gh:#1431 differential cases into
# ./inputs/. Usage: make_inputs.sh
# Facilities are built in the order listed in each case, which fixes agent
# ids and therefore the order in which the exchange sees them.
set -eu
cd "$(dirname "$0")"
mkdir -p inputs

# source NAME THROUGHPUT INVENTORY(or "")
src() {
  echo "  <facility><name>$1</name><config><Source><outcommod>u</outcommod><throughput>$2</throughput>${3:+<inventory_size>$3</inventory_size>}</Source></config></facility>"
}
# sink NAME CAPACITY MAXINV(or "") PREF(or "")
snk() {
  echo "  <facility><name>$1</name><config><Sink><in_commods><val>u</val></in_commods>${4:+<in_commod_prefs><val>$4</val></in_commod_prefs>}<capacity>$2</capacity>${3:+<max_inv_size>$3</max_inv_size>}</Sink></config></facility>"
}
# case FILE FACILITYDEFS... -- ENTRY NAMES...
mk() {
  local out=$1; shift
  local defs=() names=() sep=0
  for a in "$@"; do
    if [ "$a" = "--" ]; then sep=1; continue; fi
    if [ $sep = 0 ]; then defs+=("$a"); else names+=("$a"); fi
  done
  {
    echo '<simulation>'
    echo '  <control><duration>24</duration><startmonth>1</startmonth><startyear>2000</startyear></control>'
    echo '  <archetypes>'
    echo '    <spec><lib>cycamore</lib><name>Source</name></spec>'
    echo '    <spec><lib>cycamore</lib><name>Sink</name></spec>'
    echo '    <spec><lib>agents</lib><name>NullRegion</name></spec>'
    echo '    <spec><lib>agents</lib><name>NullInst</name></spec>'
    echo '  </archetypes>'
    for d in "${defs[@]}"; do echo "$d"; done
    echo '  <region><name>R</name><config><NullRegion/></config>'
    echo '    <institution><name>I</name><initialfacilitylist>'
    for n in "${names[@]}"; do echo "      <entry><prototype>$n</prototype><number>1</number></entry>"; done
    echo '    </initialfacilitylist><config><NullInst/></config></institution>'
    echo '  </region>'
    echo '</simulation>'
  } > "inputs/$out"
}

mk c1a.xml "$(src S 100 1000)" "$(snk K 80 '' '')" -- S K
mk c1b.xml "$(src S 33.3 '')" "$(snk K 100 500 '')" -- S K
mk c2a.xml "$(src S 100 '')" "$(snk A 60 '' '')" "$(snk B 70 '' '')" -- S A B
mk c2b.xml "$(src S 100 '')" "$(snk A 60 '' 1)" "$(snk B 70 '' 2)" -- S A B
mk c2c.xml "$(src P 60 '')" "$(src Q 70 '')" "$(snk K 100 '' '')" -- P Q K
