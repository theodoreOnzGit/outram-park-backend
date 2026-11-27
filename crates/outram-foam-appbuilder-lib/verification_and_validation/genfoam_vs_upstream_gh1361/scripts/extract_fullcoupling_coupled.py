#!/usr/bin/env python3
"""gh:#1856 round 56: references of the real coupled 2D_fullCoupling steady state (make_fullcoupling_coupled_case.sh + run_fullcoupling_coupled.sh).

usage: extract_fullcoupling_coupled.py <run dir> <out.txt>

Text lines (all numbers as the binary wrote them, 17 digits):
  STEP <k> <time name> <dt of this step>          dt = 1 for step 1, else the log's `deltaT =` line of step k-1 (17 digits)
  DISP <k> x y z (a single triple = uniform) x y z ...                          neutroRegion `disp` cell values, EVERY step (the displacement the step's deformMesh() read)
  LOG  <k> keffprev keff power                      from the log: `keff_ previous`, `keff_`, `power:` (every step)
  and, for steps 1-30, every 10th after and the last (the "kept" steps):
  PTS  <k> x y z ...        neutroRegion/polyMesh/points (the moved points)
  F0 / F1 / PD / MPHI <k> ...   flux0, flux1, powerDensity, meshPhi (internalField list; `uniform v` expanded to one value)
"""
import os, re, sys

def read_list(path):
    t = open(path).read()
    m = re.search(r'internalField\s+(uniform\s+([^;]+);|nonuniform\s+List<\w+>\s*(\d+)\s*\()', t)
    if m.group(2) is not None:
        return None, m.group(2).replace('(', ' ').replace(')', ' ').split()
    n = int(m.group(3)); body = t[m.end():]
    body = body[:body.index('\n)')]
    return n, body.replace('(', ' ').replace(')', ' ').split()

def read_points(path):
    t = open(path).read()
    i = t.index('\n(') + 2
    body = t[i:t.rindex('\n)')]
    return body.replace('(', ' ').replace(')', ' ').split()

def main():
    run, out = sys.argv[1], sys.argv[2]
    times = sorted((d for d in os.listdir(run) if re.fullmatch(r'[0-9.]+', d) and d != '0'), key=float)
    log = open(os.path.join(run, 'log.GeN-Foam')).read()
    prev = re.findall(r'keff_ previous = (\S+)', log)
    keff = re.findall(r'keff_ = (\S+)', log)
    power = re.findall(r'^power: (\S+)', log, re.M)
    assert len(prev) == len(keff) == len(power) == len(times), (len(prev), len(keff), len(power), len(times))
    dtnext = re.findall(r'^deltaT = (\S+)', log, re.M)
    o = open(out, 'w')
    n = len(times)
    for k, tn in enumerate(times, 1):
        d = os.path.join(run, tn)
        ut = open(os.path.join(d, 'uniform', 'time')).read()
        dt = '1' if k == 1 else dtnext[k-2]
        o.write(f'STEP {k} {tn} {dt}\n')
        _, v = read_list(os.path.join(d, 'neutroRegion', 'disp'))
        o.write('DISP %d %s\n' % (k, ' '.join(v)))
        o.write(f'LOG {k} {prev[k-1]} {keff[k-1]} {power[k-1]}\n')
        if k <= 30 or k % 10 == 0 or k == n:
            o.write('PTS %d %s\n' % (k, ' '.join(read_points(os.path.join(d, 'neutroRegion', 'polyMesh', 'points')))))
            for tag, name in (('F0', 'flux0'), ('F1', 'flux1'), ('PD', 'powerDensity'), ('MPHI', 'meshPhi')):
                nn, v = read_list(os.path.join(d, 'neutroRegion', name))
                o.write(f'{tag} {k} ' + ' '.join(v) + '\n')
    o.close()

main()
