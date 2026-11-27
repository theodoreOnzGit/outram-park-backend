#!/usr/bin/env python3
"""gh:#1929 / gh:#1928 / gh:#1361 round 65: variants of `cases/fullcoupling_fluid` that isolate the 2D_FFTF turbulence boundary conditions
the port gained (each is a small case the REAL binary runs in seconds). Every variant takes the base case, switches it to
`simulationType RAS; RASModel porousKEpsilon` with the FFTF numbers (`convergenceLength 0.1`, `turbulenceIntensityCoeff 0.16`,
`turbulenceIntensityExp -0.125`, `turbulenceLengthScaleCoeff 0.07`, `DhStruct 1`; k 1, epsilon 200 as the shipped `0/fluidRegion`;
relaxation 0.6 for `k.*` and `epsilon.*` as the shipped fvSolution) and then differs ONLY in the patch types of k/epsilon/nut/alphat/U:

  w8_zg      : `fixedWalls` U `noSlip` (the FFTF `vesselWall`), k/epsilon/nut/alphat `zeroGradient` on EVERY patch (FFTF: no wall functions)
  w9_calc    : `fixedWalls` U `slip` (the FFTF baffles), nut and alphat `calculated value uniform 0` on `fixedWalls`, alphat `calculated` on
               `inlet`/`outlet` too (FFTF: baffle1..4 and the two secondary-loop patches), k/epsilon `zeroGradient` everywhere
  w10_mapped : `fixedWalls` split into two patches of 24 faces typed `mappedWall` (FFTF `baffle3`/`baffle4`: `sampleMode nearestPatchFace;
               sampleRegion fluid; samplePatch <the other>`), U `slip`, nut and alphat `calculated`, k/epsilon `zeroGradient`

usage: make_fftf_turb_variants.py <base case dir> <out dir>
"""
import os, re, shutil, sys

HDR = '/*--------------------------------*- C++ -*----------------------------------*\\\n| round 65 variant (gh:#1929)                                                    |\n\\*---------------------------------------------------------------------------*/\n'


def edit(path, f):
    t = open(path).read()
    n = f(t)
    assert n != t, path
    open(path, 'w').write(n)


def field(name, dims, internal, patches):
    body = ''.join('    %s\n    {\n%s    }\n' % (p, ''.join('        %-15s %s;\n' % kv for kv in bc)) for p, bc in patches)
    return (HDR + 'FoamFile\n{\n    version 2.0;\n    format ascii;\n    class volScalarField;\n    object %s;\n}\n\n' % name
            + 'dimensions      %s;\n\ninternalField   uniform %s;\n\nboundaryField\n{\n%s}\n' % (dims, internal, body))


THERMO = '''thermoType
{
    type                heRhoThermo;
    mixture             pureMixture;
    equationOfState     Boussinesq;
    thermo              hConst;
    transport           const;
    specie              specie;
    energy              sensibleEnthalpy;
}

pRef        100000;

//- Properties are those of liquid Sodium evaluated at 600 K (shipped 2D_FFTF)
mixture
{
    specie
    {
        molWeight   22.989769;
    }
    equationOfState
    {
        rho0    874;
        T0      600;
        beta    2.602975e-4;
    }
    thermodynamics
    {
        Cp      1301;
        Hf      0;
    }
    transport
    {
        mu      3.21e-4;
        Pr      5.6664993e-3;
    }
}
'''

ZG = (('type', 'zeroGradient'),)
CALC = (('type', 'calculated'), ('value', 'uniform 0'))
EMPTY = (('type', 'empty'),)


def write_fields(d, kinds):
    """kinds: patch name -> (k/epsilon bc, nut bc, alphat bc)"""
    out = os.path.join(d, '0/fluidRegion')
    spec = (('k', '[0 2 -2 0 0 0 0]', '1', 0), ('epsilon', '[0 2 -3 0 0 0 0]', '200', 0), ('nut', '[0 2 -1 0 0 0 0]', '0', 1), ('alphat', '[1 -1 -1 0 0 0 0]', '0', 2))
    for name, dims, internal, idx in spec:
        pl = [(p, kinds[p][idx]) for p in kinds] + [('frontAndBack', EMPTY)]
        open(os.path.join(out, name), 'w').write(field(name, dims, internal, pl))


def main():
    base, out = sys.argv[1], sys.argv[2]
    names = ['w8_zg', 'w9_calc', 'w10_mapped']
    for v in names:
        d = os.path.join(out, v)
        shutil.rmtree(d, ignore_errors=True)
        os.makedirs(d)
        for sub in ('0', 'constant', 'system'):
            shutil.copytree(os.path.join(base, sub), os.path.join(d, sub))
        edit(os.path.join(d, 'constant/fluidRegion/turbulenceProperties'), lambda t: t.replace(
            'simulationType  laminar;',
            'simulationType RAS;\n\nRAS\n{\n    RASModel        porousKEpsilon;\n    turbulence      on;\n    printCoeffs     on;\n}\n\n'
            'porousKEpsilonProperties\n{\n    "zone0"\n    {\n        convergenceLength           0.1;\n        turbulenceIntensityCoeff    0.16;\n'
            '        turbulenceIntensityExp      -0.125;\n        turbulenceLengthScaleCoeff  0.07;\n        DhStruct                    1;\n    }\n}\n'))
        # the FFTF fluid: Boussinesq / hConst / const sodium (the port's `porousKEpsilon` is Boussinesq-only, as upstream's FFTF case)
        edit(os.path.join(d, 'constant/fluidRegion/thermophysicalProperties'), lambda t: t[:t.index('thermoType')] + THERMO)
        edit(os.path.join(d, 'system/fluidRegion/fvSolution'), lambda t: re.sub(
            r'("\.\*"\s+1;)', r'\1\n        "epsilon.*"     0.6;\n        "k.*"           0.6;', t, count=1))

    def noslip(t):
        m = re.search(r'(fixedWalls\s*\{\s*type\s+)slip;', t)
        assert m
        return t[:m.start()] + m.group(1) + 'noSlip;' + t[m.end():]
    # w8_zg
    d = os.path.join(out, 'w8_zg')
    edit(os.path.join(d, '0/fluidRegion/U'), noslip)
    write_fields(d, {'inlet': (ZG, ZG, ZG), 'outlet': (ZG, ZG, ZG), 'fixedWalls': (ZG, ZG, ZG)})
    # w9_calc
    d = os.path.join(out, 'w9_calc')
    write_fields(d, {'inlet': (ZG, ZG, CALC), 'outlet': (ZG, ZG, CALC), 'fixedWalls': (ZG, CALC, CALC)})
    # w10_mapped: split fixedWalls (startFace 270, 48 faces) into two mappedWall patches
    d = os.path.join(out, 'w10_mapped')

    def split(t):
        m = re.search(r'\n\s*fixedWalls\s*\{[^}]*\}', t)
        assert m
        blk = lambda n, s, o: ('\n    %s\n    {\n        type            mappedWall;\n        inGroups        3(wall mappedPatch baffleFaces);\n'
                               '        nFaces          24;\n        startFace       %d;\n        sampleMode      nearestPatchFace;\n'
                               '        sampleRegion    fluid;\n        samplePatch     %s;\n        offsetMode      uniform;\n        offset          (0 0 0);\n    }') % (n, s, o)
        return t[:m.start()] + blk('baffle3', 270, 'baffle4') + blk('baffle4', 294, 'baffle3') + t[m.end():]
    edit(os.path.join(d, 'constant/fluidRegion/polyMesh/boundary'), split)
    edit(os.path.join(d, 'constant/fluidRegion/polyMesh/boundary'), lambda t: t.replace('\n4\n(', '\n5\n(', 1))
    write_fields(d, {'inlet': (ZG, ZG, CALC), 'outlet': (ZG, ZG, CALC), 'baffle3': (ZG, CALC, CALC), 'baffle4': (ZG, CALC, CALC)})
    # the other fields of the base case name `fixedWalls`: duplicate the entry for the two new patches
    for f in ('T', 'U', 'p', 'p_rgh', 'powerDensityNeutronics'):
        def dup(t):
            m = re.search(r'\n(\s*)fixedWalls\s*\{([^}]*)\}', t)
            assert m, f
            ind, body = m.group(1), m.group(2)
            return t[:m.start()] + ''.join('\n%s%s\n%s{%s}' % (ind, n, ind, body) for n in ('baffle3', 'baffle4')) + t[m.end():]
        edit(os.path.join(d, '0/fluidRegion', f), dup)


main()
