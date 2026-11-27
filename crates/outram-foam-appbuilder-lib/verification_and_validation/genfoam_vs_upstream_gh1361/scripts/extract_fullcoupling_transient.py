#!/usr/bin/env python3
"""gh:#1919 / #1923 / gh:#1361 round 60: references of the real 2D_fullCoupling steady state (Allrun time control, last step lands on t = 100) and of the
as-shipped DIFFUSION transient restarted from it (variant SH of README round 59), every step.

usage: extract_fullcoupling_transient.py ss <ss run dir> <out.txt>
       extract_fullcoupling_transient.py tr <tr run dir> <out.txt>
       extract_fullcoupling_transient.py pk <pk run dir> <out.txt>      (gh:#1922 round 61: the POINT-KINETICS variant, `regionSolvers.neutroRegion pointKinetics`)
(run dirs made by make_fullcoupling_coupled_case.sh (ADJ=1) / make_fullcoupling_transient_case.sh (SHIPPED=1), run by run_fullcoupling_coupled.sh /
run_fullcoupling_transient.sh)

Text lines (numbers as the binary wrote them, 17 digits):
  STEP <k> <time name> <dt used by this step>        dt of step 1 = the controlDict deltaT, else the log's `deltaT =` line of step k-1
  LOG  <k> keffprev keff power maxSigmaEq            (ss) from the log
  NLOG <k> power iters finalResidual oldIntegral newIntegral    (tr) `power:`, neutron iterations, `Final neutroResidual`, integral predictor lines
  FLUX <k> n ini1 it1 ini2 it2 ...                   neutron flux solves (fluxStar0/1, two solves each, in log order)
  PRGH <k> n ...   H <k> n ...   TM <k> n ...        as round 57
  FP   <k> <region> <field> <n> <sum> <max|.|> <sum of squares>      fingerprint of every field written at step k
  PK   <k> <name> <value>                            (pk) every `name = value` line of the step's `pointKinetics:` block (spaces of the name as `_`; an arrow
                                                      line `-> TFuel` is `arrow_TFuel`; `totalPower`, `fission`, `decay`, `totalReactivity`, the terms, the averages)
  V    <k> <region> <field> v1 v2 ...                full internalField at the kept steps (tr: 1-10, every 25th, the last; ss: the last)
  PTS  <k> x y z ...                                  moved neutronics points at the kept steps (tr: step 1 and the last)
"""
import os, re, sys

FIELDS = {
    'fluidRegion': ['T', 'U', 'p', 'p_rgh', 'T.passiveStructure', 'T.fuelAvForNeutronics', 'T.cladAvForNeutronics',
                    'powerDensityNeutronics', 'powerDensityNeutronicsToLiquid'],
    'neutroRegion': ['disp', 'powerDensity', 'flux0', 'flux1', 'TCool', 'TFuel', 'TClad', 'TStructMech',
                     'prec0', 'prec1', 'prec2', 'prec3', 'prec4', 'prec5', 'secondaryPowerDensity'],
    'thermoMechanicalRegion': ['disp', 'meshDisp', 'TStruct', 'TStructFromTH', 'TFuel', 'powerDensityNeutronics', 'fuelDisp', 'CRDisp'],
}
PK_NEUTRO = ['powerDensity', 'flux0', 'flux1', 'TCool', 'TFuel', 'TClad', 'TStruct', 'TStructMech', 'rhoCool', 'oneGroupFlux',
             'prec0', 'prec1', 'prec2', 'prec3', 'prec4', 'prec5', 'secondaryPowerDensity']
NCELLS = {'fluidRegion': 144, 'neutroRegion': 432, 'thermoMechanicalRegion': 432}


def read_list(path):
    t = open(path).read()
    m = re.search(r'internalField\s+(uniform\s+([^;]+);|nonuniform\s+List<\w+>\s*(\d+)\s*\()', t)
    if m.group(2) is not None:
        return None, m.group(2).replace('(', ' ').replace(')', ' ').split()
    n = int(m.group(3)); body = t[m.end():]
    body = body[:body.index('\n)')]
    return n, body.replace('(', ' ').replace(')', ' ').split()


def fields_of(run, tn, k, kept, o, mode='tr'):
    for region, names in FIELDS.items():
        if mode == 'pk' and region == 'neutroRegion':
            names = PK_NEUTRO
        for fld in names:
            p = os.path.join(run, tn, region, fld)
            if not os.path.exists(p):
                continue
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


def points(run, tn, k, o):
    p = os.path.join(run, tn, 'neutroRegion', 'polyMesh', 'points')
    if not os.path.exists(p):
        return
    pts = open(p).read()
    i = pts.index('\n(') + 2
    o.write('PTS %d %s\n' % (k, ' '.join(pts[i:pts.rindex('\n)')].replace('(', ' ').replace(')', ' ').split())))


def main():
    mode, run, out = sys.argv[1], sys.argv[2], sys.argv[3]
    times = sorted((d for d in os.listdir(run) if re.fullmatch(r'[0-9.]+', d) and d != '0'), key=float)
    log = open(os.path.join(run, 'log.GeN-Foam')).read()
    parts = re.split(r'^Time = (\S+)$', log, flags=re.M)
    steps = [(parts[i], parts[i + 1]) for i in range(1, len(parts), 2)]
    n = len(steps)
    o = open(out, 'w')
    dtnext = re.findall(r'^deltaT = (\S+)', log, re.M)
    ctl = open(os.path.join(run, 'system', 'controlDict')).read()
    dt0 = re.search(r'^deltaT\s+(\S+);', ctl, re.M).group(1)
    o.write(f'NSTEPS {n}\n')
    for k, (tn, body) in enumerate(steps, 1):
        dt = dt0 if k == 1 else dtnext[k - 2]
        o.write(f'STEP {k} {tn} {dt}\n')
        if mode == 'ss':
            prev = re.search(r'keff_ previous = (\S+)', body).group(1)
            keff = re.search(r'keff_ = (\S+)', body).group(1)
            power = re.search(r'^power: (\S+)', body, re.M).group(1)
            ms = re.search(r'Max sigmaEq = (\S+)', body); sig = ms.group(1) if ms else '-'
            o.write(f'LOG {k} {prev} {keff} {power} {sig}\n')
        elif mode == 'pk':
            blk = body[body.index('pointKinetics:'):]
            blk = blk[:blk.index('\n', blk.index('driveline (T, exp)'))]
            for line in blk.splitlines()[1:]:
                m = re.match(r'^\s+(->\s*)?(.*?)\s*=\s*(\S+)', line)
                if m:
                    nm = ('arrow_' if m.group(1) else '') + re.sub(r'[^A-Za-z0-9_]+', '_', m.group(2)).strip('_')
                    o.write(f'PK {k} {nm} {m.group(3)}\n')
        else:
            power = re.search(r'^power: (\S+)', body, re.M).group(1)
            fin = re.search(r'Final neutroResidual at neutroIter no (\d+): (\S+)', body)
            old = re.search(r'Old integral flux: (\S+)', body).group(1)
            new = re.search(r'New predicted integral flux: (\S+)', body).group(1)
            o.write(f'NLOG {k} {power} {fin.group(1)} {fin.group(2)} {old} {new}\n')
        solve = re.findall(r'Solving for (\S+), Initial residual = (\S+), Final residual = (\S+), No Iterations (\d+)', body)
        pr = [(a, c) for (f, a, b, c) in solve if f == 'p_rgh']
        hh = [(a, c) for (f, a, b, c) in solve if f == 'h']
        fl = [(a, c) for (f, a, b, c) in solve if f.startswith('fluxStar')]
        tm = [(f, a, c) for (f, a, b, c) in solve if f in ('TStruct', 'dispx', 'dispz', 'dispy', 'fuelDisp', 'CRDisp')]
        o.write(f'FLUX {k} {len(fl)} ' + ' '.join(f'{a} {c}' for a, c in fl) + '\n')
        o.write(f'PRGH {k} {len(pr)} ' + ' '.join(f'{a} {c}' for a, c in pr) + '\n')
        o.write(f'H {k} {len(hh)} ' + ' '.join(f'{a} {c}' for a, c in hh) + '\n')
        o.write(f'TM {k} {len(tm)} ' + ' '.join(f'{f} {a} {c}' for f, a, c in tm) + '\n')
        if mode in ('tr', 'pk'):
            kept = k <= 10 or k % 25 == 0 or k == n
            fields_of(run, tn, k, kept, o, mode)
            if k == 1 or k == n:
                points(run, tn, k, o)
        elif k == n:
            fields_of(run, tn, k, True, o)
            points(run, tn, k, o)
    o.close()


main()
