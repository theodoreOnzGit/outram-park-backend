#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# Proves the transcribed statements in the gh:#1356 drivers are present in the
# pinned DWSIM source.  usage: check_verbatim.sh <dwsim clone at 1abf72d1>
#  (1) statement greps: key upstream VB statements, whitespace-normalised, must
#      occur in the named file;
#  (2) literal check: every decimal literal in a driver's transcribed functions
#      must occur (numerically) in the named upstream files, which catches a
#      mistyped coefficient.
set -euo pipefail
UP=${1:?clone dir at 1abf72d1}
here=$(cd "$(dirname "$0")" && pwd)
test "$(git -C "$UP" rev-parse HEAD)" = 1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766
T=$UP/DWSIM.Thermodynamics/PropertyPackages/Models
SC=$UP/DWSIM.UnitOperations/UnitOperations/ShortcutColumn.vb
norm() { tr -d '\r' < "$1" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]\+/ /g'; }
miss=0; n=0
need() { # file, statement
  n=$((n+1))
  if ! norm "$1" | grep -F -- "$2" >/dev/null; then echo "NOT FOUND in $(basename "$1"): $2"; miss=$((miss+1)); fi
}
need $T/NRTL.vb 'Gij(i) = alpha12(i).NegateY.MultiplyY(tau_ij(i)).ExpY'
need $T/NRTL.vb 'S(i) = Vx.MultiplyY(Gji(i)).SumY'
need $T/NRTL.vb 'C(i) = Vx.MultiplyY(Gji(i)).MultiplyY(tau_ji(i)).SumY'
need $T/NRTL.vb 'lnVg = C.DivideY(S)'
need $T/NRTL.vb 'lnVg(i) += Vx.MultiplyY(Gij(i)).MultiplyY(tau_ij(i).SubtractY(C.DivideY(S))).DivideY(S).SumY'
need $T/NRTL.vb '/ (1.98721 * T)'
need $T/UNIQUAC.vb 'tau_ij(i) = a12(i).NegateY.AddY(b12(i).MultiplyConstY(T).AddY(c12(i).MultiplyConstY(T ^ 2))).MultiplyConstY(1 / (1.98721 * T)).ExpY'
need $T/UNIQUAC.vb 'fi = Vx.MultiplyY(VR).MultiplyConstY(1 / r)'
need $T/UNIQUAC.vb 'teta = Vx.MultiplyY(VQ).MultiplyConstY(1 / q)'
need $T/UNIQUAC.vb 'S(i) = teta.MultiplyY(tau_ji(i)).SumY'
need $T/UNIQUAC.vb 'sum1(i) = teta.MultiplyY(tau_ij(i).DivideY(S)).SumY'
need $T/UNIQUAC.vb 'lngc = VR.MultiplyConstY(-1 / r).AddConstY(1).AddY(VR.MultiplyConstY(1 / r).LogY.AddY(VQ.MultiplyConstY(-z / 2).MultiplyY(fi.DivideY(teta).NegateY.AddConstY(1).AddY(fi.DivideY(teta).LogY))))'
need $T/UNIQUAC.vb 'lngr = VQ.MultiplyY(S.LogY.NegateY.SubtractY(sum1).AddConstY(1))'
need $SC 'm_Nmin = Math.Log(S) / Math.Log(alpha(lki) / alpha(hki))'
need $SC 'cte(i) = 10 ^ (m_Nmin * Math.Log10(alpha(i)) + C)'
need $SC 'yy = 0.75 * (1 - xx ^ 0.5668)'
need $SC 'm_N = (yy + m_Nmin) / (1 - yy)'
need $SC 'Ns = NminS * m_N / m_Nmin'
need $SC 'Return (value - 1 + q) ^ 2'
need $UP/DWSIM.Math/BrentMinimize.vb 'If Not (Math.Abs(p) >= Math.Abs(0.5 * q * etemp) Or p <= q * (ia - x) Or p >= q * (ib - x)) Then'
need $UP/DWSIM.MathOps.Mapack/LuDecomposition.cs 'LUrowi[j] = LUcolj[i] -= s;'

# (2) literal check, done numerically (upstream writes 1e6 as 1000000.0 etc.)
lit_rc=0
python3 -I - "$here" "$UP" <<'PY' || lit_rc=$?
import re, sys, os
here, up = sys.argv[1:3]
def literals(text):
    return {float(m) for m in re.findall(r'\d+\.\d+(?:[eE][-+]?\d+)?|\d+[eE][-+]?\d+', text)}
def slurp(*paths): return ''.join(open(os.path.join(up, p), errors='ignore').read() for p in paths)
checks = [
  ('petroleum_driver.py', 'def Tc_RiaziDaubert', 'TB = ', ['DWSIM.Thermodynamics/PetroleumCharacterization/PropertyMethods.vb']),
  ('fug_driver.py', 'def brentoptimize2', 'def main', ['DWSIM.UnitOperations/UnitOperations/ShortcutColumn.vb', 'DWSIM.Math/BrentMinimize.vb']),
]
bad = 0; total = 0
for drv, start, stop, files in checks:
    src = open(os.path.join(here, drv)).read()
    body = src[src.index(start):src.index(stop)]
    body = '\n'.join(l for l in body.splitlines() if not l.lstrip().startswith('#'))
    have = literals(slurp(*files))
    for lit in sorted(literals(body)):
        if lit == 0.0: continue   # VB default initialiser, not a coefficient
        total += 1
        if lit not in have:
            print(f'LITERAL {lit} of {drv} not in upstream'); bad += 1
print(f'literal check: {total} distinct literals, {bad} not found')
sys.exit(1 if bad else 0)
PY
echo "statement check: $n items, $miss not found in upstream"
test "$miss" -eq 0 -a "$lit_rc" -eq 0
