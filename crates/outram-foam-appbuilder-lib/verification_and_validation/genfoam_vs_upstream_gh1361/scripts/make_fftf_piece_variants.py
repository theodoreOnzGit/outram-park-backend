#!/usr/bin/env python3
"""gh:#1870 / gh:#1865 / gh:#1361 round 62: one-edit variants of `cases/fullcoupling_fluid` that isolate the pieces of 2D_FFTF
the port gained (each variant is a small case the REAL binary runs in seconds):

  w1_dilu     : explicit `"(h|k|epsilon)"` and `"(h|k|epsilon)Final"` entries AFTER the catch-all, `PBiCG`/`DILU` at 1e-14, relTol 0
                (the regex key and the DILU solver of the shipped 2D_FFTF fvSolution; the real log reads `DILUPBiCG: Solving for h`)
  w2_half_cc  : `zone0` shrunk to the first 72 of the 144 cells (an interface: alpha 0.5 below, 1 above; the shipped case is uniform, which
                makes the reconstruction trivial), default `momentumMode` (the old path under an interface: a control of the old form)
  w2_recon05  : the half zone, PIMPLE `momentumMode cellCenteredFaceReconstruction; porousInterfaceSharpness 0.5;`   (FFTF's values)
  w2_recon1   : same, sharpness 1
  w2_recon0   : same, sharpness 0 (`U = reconstruct(fluid_.phi())`)
  w3_pump     : zone0 `momentumSource (0 0 50)` with `momentumSourceTimeProfile { type table; startTime 1; table table ((0 1) (2 0.5) (4 0.25)); }`
  w4_churchill: zone0 drag model `Churchill` (surfaceRoughness 1e-6) in place of the case's `ReynoldsPower`, when the case has one
  w5_noslip   : the `fixedWalls` patch U `slip` -> `noSlip`
  w6_aniso    : zone0 drag per axis as the FFTF core: `localZ` Rehme (217 pins, FFTF numbers), `localX`/`localY` ReynoldsPower 0.96 / -0.145
  w7_tort     : (round 64, gh:#1229) the half zone (72 of 144 cells; an interface) with `localTortuosity (0.7 1.3 0.01389)` on `zone0` (the
                tensor-diffusivity Laplacian of `UEqn_1p.H` and `EEqn_1p.H`); the mesh is Cartesian, so `SfGammaCorr` is exactly 0
  w7r_rot0    : (round 64) the half zone, NO tortuosity, the mesh points ROTATED by 30 degrees about z (control: the port on a rotated mesh)
  w7r_tort    : (round 64) the half zone, `localTortuosity (0.7 1.3 0.01389)`, mesh rotated 30 degrees about z: `Sf & gamma` is not parallel
                to `Sf`, so `SfGammaCorr` (the `gammaSnGradCorr` term) is non-zero
  w11_tpas    : (round 65, gh:#1928) `passiveProperties.T` 590 against the fluid `T` 600 (the shipped FFTF starts the passive structure 0.002 K below
                the fluid; the shipped 2D_fullCoupling at the same temperature, which hides an initial-state difference)

usage: make_fftf_piece_variants.py <base case dir> <out dir>
"""
import os, re, shutil, sys

def edit(path, f):
    t = open(path).read()
    n = f(t)
    assert n != t, path
    open(path, 'w').write(n)

def main():
    base, out = sys.argv[1], sys.argv[2]
    names = ['w1_dilu', 'w2_half_cc', 'w2_recon05', 'w2_recon1', 'w2_recon0', 'w3_pump', 'w4_churchill', 'w5_noslip', 'w6_aniso', 'w7_tort', 'w7r_rot0', 'w7r_tort', 'w11_tpas']
    for v in names:
        d = os.path.join(out, v)
        shutil.rmtree(d, ignore_errors=True)
        os.makedirs(d)
        for sub in ('0', 'constant', 'system'):
            shutil.copytree(os.path.join(base, sub), os.path.join(d, sub))
    fv = 'system/fluidRegion/fvSolution'
    def dilu(t):
        entry = lambda n: f'    "{n}"\n    {{\n        solver          PBiCG;\n        preconditioner  DILU;\n        tolerance       1e-14;\n        relTol          0;\n    }}\n'
        i = t.index('".*"'); close = t.index('}', i); nl = t.index('\n', close) + 1
        return t[:nl] + entry('(h|k|epsilon)') + entry('(h|k|epsilon)Final') + t[nl:]
    edit(os.path.join(out, 'w1_dilu', fv), dilu)
    def half(t):
        m = re.search(r'(cellLabels\s+List<label>\s*)\n(\d+)\n\(\n(.*?)\n\)', t, flags=re.S)
        labels = m.group(3).split('\n')
        assert len(labels) == int(m.group(2)) == 144
        return t[:m.start()] + m.group(1) + '\n72\n(\n' + '\n'.join(labels[:72]) + '\n)' + t[m.end():]
    for v in ('w2_half_cc', 'w2_recon05', 'w2_recon1', 'w2_recon0'):
        edit(os.path.join(out, v, 'constant/fluidRegion/polyMesh/cellZones'), half)
    for v, s in (('w2_recon05', '0.5'), ('w2_recon1', '1'), ('w2_recon0', '0')):
        edit(os.path.join(out, v, fv), lambda t, s=s: re.sub(r'(PIMPLE\s*\{)', r'\1\n    momentumMode                cellCenteredFaceReconstruction;\n    porousInterfaceSharpness    ' + s + ';', t, count=1))
    pp = 'constant/fluidRegion/phaseProperties'
    edit(os.path.join(out, 'w3_pump', pp), lambda t: re.sub(r'(volumeFraction\s+0\.5;)', r'\1\n        momentumSource      (0 0 50);\n        momentumSourceTimeProfile\n        {\n            type        table;\n            startTime   1;\n            table       table\n            (\n                (0 1)\n                (2 0.5)\n                (4 0.25)\n            );\n        }', t, count=1))
    def churchill(t):
        m = re.search(r'dragModels\s*\{\s*"zone0"\s*\{[^}]*\}', t)
        assert m, 'no dragModels.zone0'
        return t[:m.start()] + 'dragModels\n    {\n        "zone0"\n        {\n            type                Churchill;\n            surfaceRoughness    1e-6;\n        }' + t[m.end():]
    edit(os.path.join(out, 'w4_churchill', pp), churchill)
    def noslip(t):
        m = re.search(r'(fixedWalls\s*\{\s*type\s+)slip;', t)
        assert m
        return t[:m.start()] + m.group(1) + 'noSlip;' + t[m.end():]
    edit(os.path.join(out, 'w5_noslip', '0/fluidRegion/U'), noslip)

    def aniso(t):
        m = re.search(r'dragModels\s*\{\s*"zone0"\s*\{[^}]*\}', t)
        assert m
        body = ('dragModels\n    {\n        "zone0"\n        {\n'
                '            "localZ"\n            {\n                type                Rehme;\n                numberOfPins        217;\n'
                '                pinDiameter         0.005842;\n                wireDiameter        0.001445;\n                wireLeadLen         0.302;\n'
                '                wetWrapPerimeter    0.38143;\n            }\n'
                '            "localX"\n            {\n                type    ReynoldsPower;\n                coeff   0.96;\n                exp     -0.145;\n            }\n'
                '            "localY"\n            {\n                type    ReynoldsPower;\n                coeff   0.96;\n                exp     -0.145;\n            }\n        }')
        return t[:m.start()] + body + t[m.end():]
    edit(os.path.join(out, 'w6_aniso', pp), aniso)

    # round 64 (gh:#1229): the tortuous zone, on the Cartesian and on the rotated mesh
    for v in ('w7_tort', 'w7r_rot0', 'w7r_tort'):
        edit(os.path.join(out, v, 'constant/fluidRegion/polyMesh/cellZones'), half)
    for v in ('w7_tort', 'w7r_tort'):
        edit(os.path.join(out, v, pp), lambda t: re.sub(r'(volumeFraction\s+0\.5;)', r'\1\n        localTortuosity     (0.7 1.3 0.01389);', t, count=1))
    import math
    c, s = math.cos(math.radians(float(os.environ.get("ROT_DEG", "30")))), math.sin(math.radians(float(os.environ.get("ROT_DEG", "30"))))
    def rotate(t):
        m = re.search(r'\n(\d+)\n\(\n(.*?)\n\)', t, flags=re.S)
        pts = []
        for line in m.group(2).split('\n'):
            x, y, z = (float(w) for w in line.strip('()').split())
            pts.append('(%.17g %.17g %.17g)' % (c * x - s * y, s * x + c * y, z))
        assert len(pts) == int(m.group(1))
        return t[:m.start()] + '\n' + m.group(1) + '\n(\n' + '\n'.join(pts) + '\n)' + t[m.end():]
    for v in ('w7r_rot0', 'w7r_tort'):
        edit(os.path.join(out, v, 'constant/fluidRegion/polyMesh/points'), rotate)
    edit(os.path.join(out, 'w11_tpas', pp), lambda t: re.sub(r'(passiveProperties\s*\{[^}]*?\bT\s+)600;', r'\g<1>590;', t, count=1))


if __name__ == '__main__':
    main()
