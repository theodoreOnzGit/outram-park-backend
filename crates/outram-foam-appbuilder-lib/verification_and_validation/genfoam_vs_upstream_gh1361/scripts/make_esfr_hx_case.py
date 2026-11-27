# gh:#1903 README "Round 52": builds cases/esfr_hx, the 3-D cell-centred heat-exchanger case: the 3D_SmallESFR fluidRegion MESH (shipped, 23322 cells)
# with the 2D_MSFR steadyStateTH SETTINGS (the only thermo/closures the cell-centred region ports), two added cellZones hxA/hxB that split the core
# (innerCore + outerCore) into two halves of near-equal volume, and a nonuniform initial T (900 K, 800 K in hxB) so the exchangers have a driving
# difference. Rule fixed BEFORE any run: cells of the core ordered by (round(x,6), round(y,6), round(z,6)) of the cell centre, cut where the two
# volumes balance best; hxA = the first part. Nothing is tuned: no number below was chosen after seeing a result.
# usage: python3 -I make_esfr_hx_case.py [out dir]   (idempotent; reads the shipped tutorial under /home/user/upstream-build)
import gzip, math, os, re, shutil, sys

HERE = os.path.dirname(os.path.abspath(__file__))
VV = os.path.dirname(HERE)
SRC = '/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/newSolver'
BASE = VV + '/cases/msfr_th'
OUT = sys.argv[1] if len(sys.argv) > 1 else VV + '/cases/esfr_hx'
M = SRC + '/constant/fluidRegion/polyMesh'


def body(p):
    t = gzip.open(p, 'rt').read()
    m = re.search(r'\n(\d+)\s*\n\(', t)
    return int(m.group(1)), t[m.end():]


n, t = body(M + '/points.gz')
pts = [tuple(map(float, x.split())) for x in re.findall(r'\(([^()]+)\)', t[:t.rindex(')')])][:n]
n, t = body(M + '/owner.gz')
own = list(map(int, t.split(')')[0].split()))
n, t = body(M + '/neighbour.gz')
nei = list(map(int, t.split(')')[0].split()))
n, t = body(M + '/faces.gz')
faces = [list(map(int, x.split())) for x in re.findall(r'\d+\(([^()]+)\)', t)]
nc = max(own) + 1
V = [0.0] * nc
cc = [[0.0] * 3 for _ in range(nc)]
cnt = [0] * nc
for fi, f in enumerate(faces):
    p0 = [sum(pts[i][k] for i in f) / len(f) for k in range(3)]
    S = [0.0, 0.0, 0.0]
    for a in range(len(f)):
        A = pts[f[a]]
        B = pts[f[(a + 1) % len(f)]]
        u = [A[k] - p0[k] for k in range(3)]
        v = [B[k] - p0[k] for k in range(3)]
        S[0] += 0.5 * (u[1] * v[2] - u[2] * v[1])
        S[1] += 0.5 * (u[2] * v[0] - u[0] * v[2])
        S[2] += 0.5 * (u[0] * v[1] - u[1] * v[0])
    d = sum(S[k] * p0[k] for k in range(3)) / 3.0
    for c, sg in [(own[fi], 1.0)] + ([(nei[fi], -1.0)] if fi < len(nei) else []):
        V[c] += sg * d
        cnt[c] += 1
        for k in range(3):
            cc[c][k] += p0[k]
for c in range(nc):
    for k in range(3):
        cc[c][k] /= cnt[c]

zt = gzip.open(M + '/cellZones.gz', 'rt').read()
zones = []
for m in re.finditer(r'\n(\w+)\s*\{\s*type cellZone;\s*cellLabels\s+List<label>\s*(\d+)\s*\(([^)]*)\)', zt):
    zones.append((m.group(1), list(map(int, m.group(3).split()))))
zd = dict(zones)
core = sorted(zd['innerCore'] + zd['outerCore'], key=lambda c: (round(cc[c][0], 6), round(cc[c][1], 6), round(cc[c][2], 6)))
tot = sum(V[c] for c in core)
best, acc, cut = None, 0.0, 0
for i, c in enumerate(core[:-1]):
    acc += V[c]
    e = abs(acc - (tot - acc))
    if best is None or e < best:
        best, cut = e, i + 1
hxA, hxB = sorted(core[:cut]), sorted(core[cut:])
vA, vB = sum(V[c] for c in hxA), sum(V[c] for c in hxB)
print('hxA', len(hxA), vA, 'hxB', len(hxB), vB, '(pV-sV)/pV AB', (vA - vB) / vA, 'BA', (vB - vA) / vB)

for d in ['constant/fluidRegion/polyMesh', 'system/fluidRegion', '0/fluidRegion']:
    os.makedirs(OUT + '/' + d, exist_ok=True)
pm = OUT + '/constant/fluidRegion/polyMesh/'
for f in ['points.gz', 'faces.gz', 'owner.gz', 'neighbour.gz']:
    shutil.copy(M + '/' + f, pm + f)
shutil.copy(M + '/boundary', pm + 'boundary')
hdr = 'FoamFile\n{\n    version     2.0;\n    format      ascii;\n    class       regIOobject;\n    location    "constant/fluidRegion/polyMesh";\n    object      cellZones;\n}\n\n'
allz = zones + [('hxA', hxA), ('hxB', hxB)]
txt = hdr + '%d\n(\n' % len(allz)
for name, ids in allz:
    txt += '%s\n{\n    type cellZone;\n    cellLabels      List<label> \n%d\n(\n%s\n)\n;\n}\n\n' % (name, len(ids), '\n'.join(map(str, ids)))
txt += ')\n'
with gzip.GzipFile(pm + 'cellZones.gz', 'wb', mtime=0) as g:
    g.write(txt.encode())

for f in ['constant/fluidRegion/thermophysicalProperties', 'constant/fluidRegion/g', 'system/controlDict', 'system/fluidRegion/fvSchemes', 'system/fluidRegion/fvSolution']:
    shutil.copy(BASE + '/' + f, OUT + '/' + f)
tp = open(BASE + '/constant/fluidRegion/turbulenceProperties').read().replace('"hx"', '"hxA:hxB"')
open(OUT + '/constant/fluidRegion/turbulenceProperties', 'w').write(tp)

pp = open(BASE + '/constant/fluidRegion/phaseProperties').read()
head = pp[:pp.index('structureProperties')]
pp_new = head + '''structureProperties
{
    "axialReflector:radialReflector:follower:controlRod"
    {
        volumeFraction      0;
        Dh                  1;
    }

    "hxA:hxB"
    {
        volumeFraction      0.6;
        Dh                  0.01;
    }

    "diagrid"
    {
        volumeFraction      0;
        Dh                  1;
        momentumSource      (0 0 300000);
    }

    heatExchangers
    {
        "ABHX"
        {
            primary             "hxA";
            secondary           "hxB";
            volumetricArea      250;
            wallConductance     2.5e4;
        }
        "BAHX"
        {
            primary             "hxB";
            secondary           "hxA";
            volumetricArea      250;
            wallConductance     2.5e4;
        }
    }
}

regimeMapModels
{}

physicsModels
{
    dragModels
    {
        "hxA:hxB"
        {
            type    ReynoldsPower;
            coeff   0.5;
            exp     -0.25;
        }
    }

    heatTransferModels
    {
        "hxA:hxB"
        {
            type    constant;
            value   1e+4;
        }
    }
}

pMin                    10000;
pRefCell                0;
pRefValue               100000;
'''
open(OUT + '/constant/fluidRegion/phaseProperties', 'w').write(pp_new)
open(OUT + '/constant/regionProperties', 'w').write('FoamFile\n{\n    version 2.0;\n    format ascii;\n    class dictionary;\n    object regionProperties;\n}\n\nregions\n(\n    fluid       (fluid)\n    solid       ()\n);\n')
open(OUT + '/constant/multiRegionCouplingDict', 'w').write('FoamFile\n{\n    version 2.0;\n    format ascii;\n    class dictionary;\n    object multiRegionCouplingDict;\n}\n\nmappings\n{\n    fluidRegion\n    {\n    }\n}\n')

patches = ['top', 'bottom', 'walls', 'baffle0', 'baffle1']


def field(name, cls, dims, internal, bc, extra=''):
    s = 'FoamFile\n{\n    version     2.0;\n    format      ascii;\n    class       %s;\n    object      %s;\n}\n\ndimensions      %s;\n\ninternalField   %s\n\nboundaryField\n{\n' % (cls, name, dims, internal)
    for p in patches:
        s += '    %s\n    {\n        %s\n    }\n' % (p, bc)
    return s + '}\n'


w = lambda n, s: open(OUT + '/0/fluidRegion/' + n, 'w').write(s)
Tv = [900.0] * nc
for c in hxB:
    Tv[c] = 800.0
w('T', field('T', 'volScalarField', '[0 0 0 1 0 0 0]', 'nonuniform List<scalar>\n%d\n(\n%s\n)\n;' % (nc, '\n'.join(repr(x) for x in Tv)), 'type            zeroGradient;'))
w('U', field('U', 'volVectorField', '[0 1 -1 0 0 0 0]', 'uniform (0 0 0);', 'type fixedValue;\n        value uniform (0 0 0);'))
w('p', field('p', 'volScalarField', '[1 -1 -2 0 0 0 0]', 'uniform 100000;', 'type            calculated;\n        value           $internalField;'))
w('p_rgh', field('p_rgh', 'volScalarField', '[1 -1 -2 0 0 0 0]', 'uniform 0;', 'type            fixedFluxPressure;\n        value           $internalField;'))
w('nut', field('nut', 'volScalarField', '[0 2 -1 0 0 0 0]', 'uniform 0;', 'type            nutkWallFunction;\n        value           uniform 0.1;'))
w('alphat', field('alphat', 'volScalarField', '[1 -1 -1 0 0 0 0]', 'uniform 0;', 'type            zeroGradient;'))
w('k', field('k', 'volScalarField', '[0 2 -2 0 0 0 0]', 'uniform 0.1;', 'type            kqRWallFunction;\n        value           uniform 0.1;'))
w('epsilon', field('epsilon', 'volScalarField', '[0 2 -3 0 0 0 0]', 'uniform 0.01;', 'type            epsilonWallFunction;\n        value           uniform 0.1;'))
print('written', OUT)
