#!/usr/bin/env python3
"""gh:#1944 round 74: STREAMING reference extractor for the real coupled 3D_SmallESFR run (a 3-D write is ~28 MB a step, 200 steps would not fit the disk).

usage: extract_esfr_coupled_stream.py <run dir> <out prefix> <last step> [kept steps, default 1,2,3,5,10,20,50,100,150,200]

While `GeN-Foam` runs in <run dir> (writeControl timeStep, writeInterval 1, writePrecision 17, log.GeN-Foam), step k's directory is COMPLETE once the log
has the line `Time = k+1` (or the run has ended); then this reads it, writes its lines, and DELETES the directory (the last one is kept).
Output files (numbers as the binary wrote them, 17 digits):
  <prefix>_log.txt   STEP k <time name> | LOG k keffprev keff power sigma | PRGH k n ini it ... | H k n ini it ... | TM k n field ini it ... |
                     FP k <region> <field> n sum max|.| sumsq   (a fingerprint of EVERY field below at EVERY step) | DT k <deltaT line of step k>
  <prefix>_V_<region>.txt   `V k <region> <field> v1 v2 ...`: the full internalField of the KEY fields at the kept steps
"""
import os, re, shutil, sys, time

FIELDS = {
    'fluidRegion': ['T', 'U', 'p', 'p_rgh', 'alphaPhi', 'htc', 'T.passiveStructure', 'T.fuelAvForNeutronics', 'T.cladAvForNeutronics',
                    'powerDensityNeutronics', 'powerDensityNeutronicsToLiquid'],
    'neutroRegion': ['disp', 'powerDensity', 'flux0', 'TCool', 'TFuel', 'TClad', 'TStructMech', 'rhoCool'],
    'thermoMechanicalRegion': ['disp', 'meshDisp', 'TStruct', 'TStructFromTH', 'TFuel', 'powerDensityNeutronics', 'fuelDisp', 'CRDisp'],
}
KEY = {
    'fluidRegion': ['T', 'U', 'p_rgh', 'T.fuelAvForNeutronics'],
    'neutroRegion': ['flux0', 'powerDensity', 'TFuel', 'disp'],
    'thermoMechanicalRegion': ['disp', 'TStruct', 'meshDisp'],
}
VECTORS = ('U', 'disp', 'meshDisp')
NCELLS = {'fluidRegion': 23322, 'neutroRegion': 23322, 'thermoMechanicalRegion': 23340}


def read_list(path):
    t = open(path).read()
    m = re.search(r'internalField\s+(uniform\s+([^;]+);|nonuniform\s+List<\w+>\s*(\d+)\s*\()', t)
    if m.group(2) is not None:
        return None, m.group(2).replace('(', ' ').replace(')', ' ').split()
    n = int(m.group(3))
    body = t[m.end():]
    body = body[:body.index('\n)')]
    return n, body.replace('(', ' ').replace(')', ' ').split()


def main():
    run, prefix, last = sys.argv[1], sys.argv[2], int(sys.argv[3])
    kept = set(int(x) for x in (sys.argv[4] if len(sys.argv) > 4 else '1,2,3,5,10,20,50,100,150,200').split(','))
    logf = os.path.join(run, 'log.GeN-Foam')
    out = open(prefix + '_log.txt', 'w')
    vout = {r: open(f'{prefix}_V_{r}.txt', 'w') for r in FIELDS}
    done = 0
    while done < last:
        time.sleep(2)
        if not os.path.exists(logf):
            continue
        log = open(logf).read()
        parts = re.split(r'^Time = (\S+)$', log, flags=re.M)
        steps = [(parts[i], parts[i + 1]) for i in range(1, len(parts), 2)]
        ended = '\nEnd\n' in log[-200:] or log.rstrip().endswith('End')
        complete = len(steps) if ended else len(steps) - 1
        dtnext = re.findall(r'^deltaT = (\S+)', log, re.M)
        while done < complete and done < last:
            k = done + 1
            tn, body = steps[k - 1]
            d = os.path.join(run, tn)
            if not os.path.isdir(d):
                break
            out.write(f'STEP {k} {tn}\n')
            m = lambda pat: re.search(pat, body, re.M)
            out.write('LOG %d %s %s %s %s\n' % (k, m(r'keff_ previous = (\S+)').group(1), m(r'keff_ = (\S+)').group(1), m(r'^power: (\S+)').group(1), m(r'Max sigmaEq = (\S+)').group(1)))
            solve = re.findall(r'Solving for (\S+), Initial residual = (\S+), Final residual = (\S+), No Iterations (\d+)', body)
            pr = [(a, c) for (f, a, b, c) in solve if f == 'p_rgh']
            hh = [(a, c) for (f, a, b, c) in solve if f == 'h']
            tm = [(f, a, c) for (f, a, b, c) in solve if f in ('TStruct', 'dispx', 'dispz', 'dispy', 'fuelDisp', 'CRDisp')]
            out.write(f'PRGH {k} {len(pr)} ' + ' '.join(f'{a} {c}' for a, c in pr) + '\n')
            out.write(f'H {k} {len(hh)} ' + ' '.join(f'{a} {c}' for a, c in hh) + '\n')
            out.write(f'TM {k} {len(tm)} ' + ' '.join(f'{f} {a} {c}' for f, a, c in tm) + '\n')
            if k - 1 < len(dtnext):
                out.write(f'DT {k} {dtnext[k - 1]}\n')
            for region, names in FIELDS.items():
                for fld in names:
                    p = os.path.join(d, region, fld)
                    if not os.path.exists(p):
                        continue
                    nn, v = read_list(p)
                    dim = 3 if fld in VECTORS else 1
                    if nn is None:
                        v = v * (NCELLS[region] if dim == 1 else 1)
                        if dim == 3:
                            v = v * NCELLS[region]
                    vals = [float(x) for x in v]
                    s = 0.0; sq = 0.0; mx = 0.0
                    for x in vals:
                        s += x; sq += x * x; mx = max(mx, abs(x))
                    out.write(f'FP {k} {region} {fld} {len(vals)} {s!r} {mx!r} {sq!r}\n')
                    if k in kept and fld in KEY[region]:
                        vout[region].write(f'V {k} {region} {fld} ' + ' '.join(v) + '\n')
            out.flush()
            for f in vout.values():
                f.flush()
            if k != last:
                shutil.rmtree(d, ignore_errors=True)
            done += 1
        if ended and done >= complete:
            break
    out.close()
    for f in vout.values():
        f.close()


main()
