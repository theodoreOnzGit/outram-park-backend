#!/bin/bash
R=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/r1635
E=/home/user/outram-park-backend/.claude/worktrees/agent-a9648db7896317606/crates/outram-mc-libs/verification_and_validation/sab_per_material_1581/ensembles_1583
ex() { grep -E "^ +seed +[0-9]+: ENDF" "$1" | awk '{print $2+0, $4, $6}'; }
ex $E/ace_roundtrip_old_d1e20f188.out > $R/o1.t
ex $E/ace_roundtrip_new_501c16264.out > $R/n1.t
ex $R/old_9_16.out > $R/o2.t
ex $R/new_9_16.out > $R/n2.t
cat $R/o1.t $R/o2.t > $R/o.t
cat $R/n1.t $R/n2.t > $R/n.t
wc -l $R/o.t $R/n.t
cat > $R/st.awk <<'EOF'
function val(c, off) { if (c == 4) return ($(off+3) - $(off+2)) * 1e5; return $(off+c) * 1e5 }
{ n++; o[n] = val(col, 0); w[n] = val(col, 3); so += o[n]; sw += w[n] }
END {
  mo = so / n; mw = sw / n
  for (i = 1; i <= n; i++) { vo += (o[i]-mo)^2; vw += (w[i]-mw)^2 }
  vo /= n - 1; vw /= n - 1
  eo = sqrt(vo / n); ew = sqrt(vw / n); sg = sqrt(eo^2 + ew^2)
  printf "N=%d old mean %.1f sd %.0f | new mean %.1f sd %.0f | F(new/old)=%.2f | new-old %+.0f +/- %.0f z %+.2f\n", n, mo, sqrt(vo), mw, sqrt(vw), vw/vo, mw-mo, sg, (mw-mo)/sg
}
EOF
for c in 2 3 4; do
  echo "col $c (2=ENDF 3=ACE 4=parity ACE-ENDF, pcm units)"
  for r in "1,8" "9,16" "1,16"; do
    echo -n "seeds $r: "
    paste <(sed -n "${r}p" $R/o.t) <(sed -n "${r}p" $R/n.t) | awk -v col=$c -f $R/st.awk
  done
done
