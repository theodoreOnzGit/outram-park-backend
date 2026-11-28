#!/usr/bin/env python3
"""gh:#1950 S4, README round 75: reference extractor for the real 3D_SmallESFR_old TRANSIENT (energyNeutroMechLoop multiPhysicsSolver).

usage: extract_esfr_transient.py <run dir> <out prefix> [kept step list, default none]
Parses log.GeN-Foam of <run dir> (a finished run, or one still running: it waits for the line `End`); for every step `Time = t`:
  STEP k <time name>
  DT k <deltaT printed after the step>
  ITER k <n multiphysics iterations> <1 if `Multiphysics loop converged` was printed else 0>
  H k n ini it ini it ...                     every `h` solve of the step, in order (all iterations)
  NEUT k j <power> <n passes> r1 r2 ...        call j (0-based, one per multiphysics iteration): `power:` and the `neutroResidual` of each pass
  FLUX k j n ini it ini it ...                 every fluxStar solve of call j
  TM k j n field ini it ...                    every thermo-mechanics solve of call j (TStruct, dispx..z, fuelDisp, CRDisp)
and, when a step's time directory exists (writeControl timeStep: kept steps only), the fingerprint of every field (n sum max|.| sumsq) and, for
the kept steps, the full internalField of the key fields (`V k <region> <field> v...`); the directory is deleted after it was read (the last one is
kept). Numbers are printed as the binary wrote them (17 digits).
"""
import os, re, shutil, sys, time

FIELDS = {
    'fluidRegion': ['T', 'U', 'p', 'p_rgh', 'alphaPhi', 'T.passiveStructure', 'T.fuelAvForNeutronics', 'T.cladAvForNeutronics', 'powerDensityNeutronics'],
    'neutroRegion': ['disp', 'powerDensity', 'flux0', 'fluxStar0', 'TCool', 'TFuel', 'TClad', 'TStructMech', 'rhoCool'],
    'thermoMechanicalRegion': ['disp', 'meshDisp', 'TStruct', 'TStructFromTH', 'TFuel', 'powerDensityNeutronics', 'fuelDisp', 'CRDisp'],
}
KEY = {
    'fluidRegion': ['T', 'U', 'p'],
    'neutroRegion': ['flux0', 'powerDensity', 'TFuel', 'disp'],
    'thermoMechanicalRegion': ['disp', 'TStruct', 'meshDisp'],
}
VECTORS = ('U', 'disp', 'meshDisp')
NCELLS = {'fluidRegion': 23322, 'neutroRegion': 23322, 'thermoMechanicalRegion': 23340}
SOLVE = re.compile(r'Solving for (\S+), Initial residual = (\S+), Final residual = (\S+), No Iterations (\d+)')


def read_list(path):
    t = open(path).read()
    m = re.search(r'internalField\s+(uniform\s+([^;]+);|nonuniform\s+List<\w+>\s*(\d+)\s*\()', t)
    if m.group(2) is not None:
        return None, m.group(2).replace('(', ' ').replace(')', ' ').split()
    n = int(m.group(3))
    body = t[m.end():]
    body = body[:body.index('\n)')]
    return n, body.replace('(', ' ').replace(')', ' ').split()


def step_lines(k, tn, body, nexts):
    out = [f'STEP {k} {tn}']
    if nexts:
        out.append(f'DT {k} {nexts}')
    calls = re.split(r'^Solving neutronics$', body, flags=re.M)
    n_iter = len(calls) - 1
    out.append(f'ITER {k} {n_iter} {1 if "Multiphysics loop converged" in body else 0}')
    hs = [(a, c) for (f, a, b, c) in SOLVE.findall(body) if f == 'h']
    out.append(f'H {k} {len(hs)} ' + ' '.join(f'{a} {c}' for a, c in hs))
    for j, call in enumerate(calls[1:]):
        pw = re.findall(r'^power: (\S+)', call, flags=re.M)
        nr = re.findall(r'^neutroResidual \(neutroIter: \d+\): (\S+)', call, flags=re.M)
        out.append(f'NEUT {k} {j} {pw[-1] if pw else "nan"} {len(nr)} ' + ' '.join(nr))
        fl = [(a, c) for (f, a, b, c) in SOLVE.findall(call) if f.startswith('fluxStar')]
        out.append(f'FLUX {k} {j} {len(fl)} ' + ' '.join(f'{a} {c}' for a, c in fl))
        tm = [(f, a, c) for (f, a, b, c) in SOLVE.findall(call) if f in ('TStruct', 'dispx', 'dispy', 'dispz', 'fuelDisp', 'CRDisp')]
        out.append(f'TM {k} {j} {len(tm)} ' + ' '.join(f'{f} {a} {c}' for f, a, c in tm))
    return out


def main():
    run, prefix = sys.argv[1], sys.argv[2]
    kept = set(int(x) for x in sys.argv[3].split(',')) if len(sys.argv) > 3 and sys.argv[3] else set()
    logf = os.path.join(run, 'log.GeN-Foam')
    out = open(prefix + '_log.txt', 'w')
    vout = {r: open(f'{prefix}_V_{r}.txt', 'w') for r in FIELDS}
    done = 0
    while True:
        if not os.path.exists(logf):
            time.sleep(2)
            continue
        log = open(logf).read()
        parts = re.split(r'^Time = (\S+)$', log, flags=re.M)
        steps = [(parts[i], parts[i + 1]) for i in range(1, len(parts), 2)]
        ended = log.rstrip().endswith('End')
        complete = len(steps) if ended else len(steps) - 1
        dts = re.findall(r'^deltaT = (\S+)', log, re.M)
        while done < complete:
            k = done + 1
            tn, body = steps[k - 1]
            d = os.path.join(run, tn)
            for line in step_lines(k, tn, body, dts[k - 1] if k - 1 < len(dts) else ''):
                out.write(line + '\n')
            if os.path.isdir(d):
                for region, names in FIELDS.items():
                    for fld in names:
                        p = os.path.join(d, region, fld)
                        if not os.path.exists(p):
                            continue
                        nn, v = read_list(p)
                        if nn is None:
                            v = v * NCELLS[region]
                        vals = [float(x) for x in v]
                        s = 0.0; sq = 0.0; mx = 0.0
                        for x in vals:
                            s += x; sq += x * x; mx = max(mx, abs(x))
                        out.write(f'FP {k} {region} {fld} {len(vals)} {s!r} {mx!r} {sq!r}\n')
                        if k in kept and fld in KEY[region]:
                            vout[region].write(f'V {k} {region} {fld} ' + ' '.join(v) + '\n')
                if not (ended and k == len(steps)):
                    shutil.rmtree(d, ignore_errors=True)
            out.flush()
            for f in vout.values():
                f.flush()
            done += 1
        if ended and done >= complete:
            break
        time.sleep(3)
    out.close()
    for f in vout.values():
        f.close()


main()
