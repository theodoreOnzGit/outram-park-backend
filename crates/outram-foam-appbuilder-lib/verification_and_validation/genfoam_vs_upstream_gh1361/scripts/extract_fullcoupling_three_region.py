#!/usr/bin/env python3
"""gh:#1915 / #1914 / gh:#1361 round 57: references of the real coupled 2D_fullCoupling steady state, ALL THREE regions, every step.

usage: extract_fullcoupling_three_region.py <run dir> <out.txt>      (run dir made by make_fullcoupling_coupled_case.sh + run_fullcoupling_coupled.sh)

Text lines (numbers as the binary wrote them, 17 digits):
  STEP <k> <time name> <dt used by this step>          dt = 1 for step 1, else the log's `deltaT =` line of step k-1 (17 digits)
  LOG  <k> keffprev keff power maxSigmaEq              from the log (every step)
  PRGH <k> <n> ini1 it1 ini2 it2 ...                    fluid `p_rgh` solves of step k in log order
  H    <k> <n> ini1 it1 ...                             fluid `h` solves
  TM   <k> <n> field1 ini1 it1 field2 ini2 it2 ...      thermoMechanical solves (TStruct, dispx, dispz, fuelDisp, CRDisp)
  FP   <k> <region> <field> <n> <sum> <max|.|> <sum of squares>      fingerprint of EVERY field below, EVERY step
  V    <k> <region> <field> v1 v2 ...                   full internalField at the kept steps (1-30, every 10th, the last)
Fields: fluidRegion T U p p_rgh T.passiveStructure T.fuelAvForNeutronics T.cladAvForNeutronics powerDensityNeutronics powerDensityNeutronicsToLiquid;
neutroRegion disp powerDensity flux0 flux1 TCool TFuel TClad TStructMech; thermoMechanicalRegion disp meshDisp TStruct TStructFromTH TFuel
powerDensityNeutronics fuelDisp CRDisp.  `uniform v` is expanded to one value (the fingerprint uses n from the mesh: the cell count).
"""
import os, re, sys

FIELDS = {
    'fluidRegion': ['T', 'U', 'p', 'p_rgh', 'T.passiveStructure', 'T.fuelAvForNeutronics', 'T.cladAvForNeutronics',
                    'powerDensityNeutronics', 'powerDensityNeutronicsToLiquid'],
    'neutroRegion': ['disp', 'powerDensity', 'flux0', 'flux1', 'TCool', 'TFuel', 'TClad', 'TStructMech'],
    'thermoMechanicalRegion': ['disp', 'meshDisp', 'TStruct', 'TStructFromTH', 'TFuel', 'powerDensityNeutronics', 'fuelDisp', 'CRDisp'],
}
NCELLS = {'fluidRegion': 144, 'neutroRegion': 432, 'thermoMechanicalRegion': 432}

def read_list(path):
    t = open(path).read()
    m = re.search(r'internalField\s+(uniform\s+([^;]+);|nonuniform\s+List<\w+>\s*(\d+)\s*\()', t)
    if m.group(2) is not None:
        return None, m.group(2).replace('(', ' ').replace(')', ' ').split()
    n = int(m.group(3)); body = t[m.end():]
    body = body[:body.index('\n)')]
    return n, body.replace('(', ' ').replace(')', ' ').split()

def main():
    run, out = sys.argv[1], sys.argv[2]
    times = sorted((d for d in os.listdir(run) if re.fullmatch(r'[0-9.]+', d) and d != '0'), key=float)
    log = open(os.path.join(run, 'log.GeN-Foam')).read()
    # split the log into steps at `Time = `
    parts = re.split(r'^Time = (\S+)$', log, flags=re.M)
    steps = [(parts[i], parts[i + 1]) for i in range(1, len(parts), 2)]
    assert len(steps) == len(times), (len(steps), len(times))
    o = open(out, 'w')
    n = len(times)
    dtnext = re.findall(r'^deltaT = (\S+)', log, re.M)
    for k, tn in enumerate(times, 1):
        name, body = steps[k - 1]
        assert name == tn, (name, tn)
        dt = '1' if k == 1 else dtnext[k - 2]
        o.write(f'STEP {k} {tn} {dt}\n')
        prev = re.search(r'keff_ previous = (\S+)', body).group(1)
        keff = re.search(r'keff_ = (\S+)', body).group(1)
        power = re.search(r'^power: (\S+)', body, re.M).group(1)
        sig = re.search(r'Max sigmaEq = (\S+)', body).group(1)
        o.write(f'LOG {k} {prev} {keff} {power} {sig}\n')
        solve = re.findall(r'Solving for (\S+), Initial residual = (\S+), Final residual = (\S+), No Iterations (\d+)', body)
        pr = [(a, c) for (f, a, b, c) in solve if f == 'p_rgh']
        hh = [(a, c) for (f, a, b, c) in solve if f == 'h']
        tm = [(f, a, c) for (f, a, b, c) in solve if f in ('TStruct', 'dispx', 'dispz', 'dispy', 'fuelDisp', 'CRDisp')]
        o.write(f'PRGH {k} {len(pr)} ' + ' '.join(f'{a} {c}' for a, c in pr) + '\n')
        o.write(f'H {k} {len(hh)} ' + ' '.join(f'{a} {c}' for a, c in hh) + '\n')
        o.write(f'TM {k} {len(tm)} ' + ' '.join(f'{f} {a} {c}' for f, a, c in tm) + '\n')
        kept = k <= 30 or k % 10 == 0 or k == n
        for region, names in FIELDS.items():
            for fld in names:
                p = os.path.join(run, tn, region, fld)
                if not os.path.exists(p):
                    continue  # not an AUTO_WRITE field (e.g. thermoMechanicalRegion/powerDensityNeutronics)
                nn, v = read_list(p)
                dim = 3 if fld in ('U', 'disp', 'meshDisp') else 1
                if nn is None:
                    v = v * (NCELLS[region] if dim == 1 else 1)
                    if dim == 3:
                        v = v * NCELLS[region]
                vals = [float(x) for x in v]
                s = 0.0; sq = 0.0; mx = 0.0
                for x in vals:
                    s += x; sq += x * x; mx = max(mx, abs(x))
                o.write(f'FP {k} {region} {fld} {len(vals)} {s!r} {mx!r} {sq!r}\n')
                if kept:
                    o.write(f'V {k} {region} {fld} ' + ' '.join(v) + '\n')
        if kept:
            pts = open(os.path.join(run, tn, 'neutroRegion', 'polyMesh', 'points')).read()
            i = pts.index('\n(') + 2
            o.write('PTS %d %s\n' % (k, ' '.join(pts[i:pts.rindex('\n)')].replace('(', ' ').replace(')', ' ').split())))
    o.close()

main()
