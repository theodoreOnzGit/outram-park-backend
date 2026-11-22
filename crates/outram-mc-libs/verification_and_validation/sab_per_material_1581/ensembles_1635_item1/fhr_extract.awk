# Extracts the seven k_eff values of one fhr_ring_rpt_endf run (stderr) into one
# line: exp_sphere exp_cube rpt_sphere rpt_cube naive_sphere naive_cube csg
# (k only; the per-solve sigma stays in the raw file). Usage:
#   awk -f fhr_extract.awk run.err
function nums(s,   a, n) { n = split(s, a, /[^0-9.]+/); return n }
/k_eff \(explicit TRISO pebble, sphere\)/ {
  match($0, /= [0-9.]+/); es = substr($0, RSTART + 2, RLENGTH - 2)
  match($0, /\[cube [0-9.]+/); ec = substr($0, RSTART + 6, RLENGTH - 6)
}
/k_eff \(ring-RPT pebble, sphere\)/ {
  match($0, /= [0-9.]+/); rs = substr($0, RSTART + 2, RLENGTH - 2)
  match($0, /\[cube [0-9.]+/); rc = substr($0, RSTART + 6, RLENGTH - 6)
}
/k_eff \(naive homogenised, sphere\)/ {
  match($0, /= [0-9.]+/); ns = substr($0, RSTART + 2, RLENGTH - 2)
  match($0, /\[cube [0-9.]+/); nc = substr($0, RSTART + 6, RLENGTH - 6)
}
/k_eff \(ring-RPT pebble, CSG sphere\)/ {
  match($0, /= [0-9.]+/); cs = substr($0, RSTART + 2, RLENGTH - 2)
}
END { print es, ec, rs, rc, ns, nc, cs }
