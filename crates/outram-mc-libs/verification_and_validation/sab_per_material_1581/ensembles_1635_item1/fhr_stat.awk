# Old-vs-new statistics for the seven solves of fhr_ring_rpt_endf.
# Input: `paste old.txt new.txt`, one row per seed, 7 k values per build
# (fhr_extract.awk order). sem = sd/sqrt(N); sigma = sqrt(sem_o^2 + sem_n^2);
# z = (new - old)/sigma.
BEGIN { split("explicit-sphere explicit-cube rpt-sphere rpt-cube naive-sphere naive-cube rpt-csg", nm, " ") }
{ n++; for (j = 1; j <= 7; j++) { o[j, n] = $j; w[j, n] = $(j + 7); so[j] += $j; sw[j] += $(j + 7) } }
END {
  for (j = 1; j <= 7; j++) {
    mo = so[j] / n; mw = sw[j] / n; vo = 0; vw = 0
    for (i = 1; i <= n; i++) { vo += (o[j, i] - mo) ^ 2; vw += (w[j, i] - mw) ^ 2 }
    vo /= n - 1; vw /= n - 1; eo = sqrt(vo / n); ew = sqrt(vw / n); sg = sqrt(eo ^ 2 + ew ^ 2)
    printf "%-16s N=%d old %.5f (sd %.0f, sem %.0f) | new %.5f (sd %.0f, sem %.0f) | new-old %+.0f +/- %.0f pcm z %+.2f\n", nm[j], n, mo, sqrt(vo) * 1e5, eo * 1e5, mw, sqrt(vw) * 1e5, ew * 1e5, (mw - mo) * 1e5, sg * 1e5, (mw - mo) / sg
  }
}
