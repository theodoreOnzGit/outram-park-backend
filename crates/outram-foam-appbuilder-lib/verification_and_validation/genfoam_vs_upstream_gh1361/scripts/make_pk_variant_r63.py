#!/usr/bin/env python3
"""gh:#1927 / gh:#1361 round 63: edit the PK transient stage made by make_fullcoupling_transient_case.sh into one variant of the point-kinetics options
(README round 63 table). Only the files listed there are edited; everything else is the round 61 stage.

usage: make_pk_variant_r63.py <variant> <transient case dir>
variants: zones zones_coolant_key ext_transient ext_monitor decay profiles restart gem
"""
import os, re, sys

variant, case = sys.argv[1], sys.argv[2]
NR = os.path.join(case, 'constant', 'neutroRegion')


def append(path, text):
    with open(path, 'a') as f:
        f.write(text)


def set_key(path, key, value):
    s = open(path).read()
    s, n = re.subn(r'^%s\s+[^;]*;' % re.escape(key), '%s %s;' % (key, value), s, flags=re.M)
    assert n == 1, (path, key, n)
    open(path, 'w').write(s)


def start_dir():
    ts = [d for d in os.listdir(case) if re.fullmatch(r'[0-9.]+', d) and d != '0']
    return sorted(ts, key=float)[0]


def reactor_state():
    return os.path.join(case, start_dir(), 'uniform', 'reactorState')


def zones():
    p = os.path.join(NR, 'polyMesh', 'cellZones')
    t = open(p).read()
    m = re.search(r'cellLabels\s+List<label>\s*\n(\d+)\n\(\n(.*?)\n\)\n;', t, re.S)
    n = int(m.group(1))
    allc = list(range(n))
    zs = {'zone0': allc, 'zoneA': allc[:n // 2], 'zoneB': allc[n // 2:], 'zoneC': [i for i in allc if i % 3 == 0]}
    names = ' '.join(zs)
    out = t[:t.index('FoamFile')]
    out += 'FoamFile\n{\n    version     2.0;\n    format      ascii;\n    arch        "LSB;label=32;scalar=64";\n    class       regIOobject;\n'
    out += '    location    "constant/neutroRegion/polyMesh";\n    object      cellZones;\n    meta\n    {\n        names           %d ( %s );\n    }\n}\n' % (len(zs), names)
    out += '\n%d\n(\n' % len(zs)
    for k, v in zs.items():
        out += '%s\n{\n    type            cellZone;\n    cellLabels      List<label> \n%d\n(\n%s\n)\n;\n}\n' % (k, len(v), '\n'.join(map(str, v)))
    out += ')\n\n// ************************************************************************* //\n'
    open(p, 'w').write(out)


def coefficients():
    nd = os.path.join(NR, 'nuclearData')
    for k, v in [('feedbackCoeffDoppler', '3e-05'), ('feedbackCoeffTClad', '-5e-07'), ('feedbackCoeffTCool', '3e-07'), ('feedbackCoeffRhoCool', '1e-06'),
                 ('feedbackCoeffTStruct', '3e-07'), ('feedbackCoeffTStructMech', '-3e-07'), ('absoluteDrivelineExpansionCoeff', '8e-05')]:
        set_key(nd, k, v)


def ext_source(mode_block, beam):
    append(os.path.join(NR, 'neutronicsProperties'), '\nexternalSourceNeutronics true;\n')
    hdr = 'FoamFile\n{\n    version 2.0;\n    format ascii;\n    class dictionary;\n    object externalSource;\n}\n'
    open(os.path.join(NR, 'externalSource'), 'w').write(hdr + mode_block + ('nuSource 20;\nbeamEnergy 6e8;\n' if beam else ''))
    if beam:
        append(os.path.join(NR, 'nuclearData'), '\nnuFission 2.43;\nenergyPerFission 2e8;\n')


set_key(os.path.join(case, 'system', 'controlDict'), 'endTime', '130')

if variant in ('zones', 'zones_coolant_key'):
    zones()
    coefficients()
    nd = os.path.join(NR, 'nuclearData')
    if variant == 'zones':
        append(nd, '\nfuelFeedbackZones ( zoneA );\ncoolFeedbackZones ( zoneB );\nstructFeedbackZones ( zoneA zoneC );\nstructMechFeedbackZones ( zoneC );\ndrivelineFeedbackZones ( zoneB zoneC );\n')
    else:
        append(nd, '\ncoolantFeedbackZones ( zoneA );\n')
elif variant == 'ext_transient':
    ext_source('externalSourceMode transient;\nexternalSourceModulationTimeProfile\n{\n    type table;\n    table table ( (100 1) (105 2) (110 0.5) (120 1.5) );\n}\n', True)
    append(reactor_state(), '\nsubcriticalIndex 1e-4;\n')
elif variant == 'ext_monitor':
    ext_source('externalSourceMode powerMonitoring;\nmodulationFactor 0.5;\npowerModulationDelay 0.01;\n', False)
    append(reactor_state(), '\nksrc 0.9999;\n')
elif variant == 'decay':
    append(reactor_state(), '\ndecayPowerTimeProfile\n{\n    type table;\n    table table ( (0 6e5) (100 6e5) (110 5.5e5) (130 4e5) );\n}\n')
elif variant == 'profiles':
    nd = os.path.join(NR, 'nuclearData')
    s = open(nd).read()
    # replace the one-row external reactivity table by a sine, add a boron table
    s, n = re.subn(r'externalReactivityTimeProfile\s*\{.*?\n\}', 'externalReactivityTimeProfile\n{\n    type sine;\n    frequency 0.05;\n    scale 1e-4;\n    level 5.7671e-4;\n}', s, count=1, flags=re.S)
    assert n == 1
    s += '\nboronReactivityTimeProfile\n{\n    type table;\n    table table ( (100 0) (110 2e-5) (130 4e-5) );\n}\n'
    open(nd, 'w').write(s)
    append(reactor_state(), '\ndecayPowerTimeProfile\n{\n    type quadraticRamp;\n    start 105;\n    duration 20;\n}\n')
elif variant == 'restart':
    nd = open(os.path.join(NR, 'nuclearData')).read()
    beta = [float(x) for x in re.search(r'^Beta\s*\(([^)]*)\)', nd, re.M).group(1).split()]
    lam = [float(x) for x in re.search(r'^lambda\s*\(([^)]*)\)', nd, re.M).group(1).split()]
    gen = float(re.search(r'^promptGenerationTime\s+(\S+);', nd, re.M).group(1))
    ptarget = float(re.search(r'^pTarget\s+(\S+);', open(reactor_state()).read(), re.M).group(1))
    pp = [1.05 * (b * ptarget) / (l * gen) for b, l in zip(beta, lam)]
    append(reactor_state(), '\nprecursorPowers %d ( %s );\nTFuelRef 700;\nTladRef 480;\nTCoolRef 470;\nrhoCoolRef 690;\nTStructRef 0;\nTStructMechRef 460;\nTDriveLineRef 0;\n' % (len(pp), ' '.join(repr(x) for x in pp)))
elif variant == 'gem':
    append(os.path.join(NR, 'neutronicsProperties'), '\nfluidRegionName fluidRegion;\n')
    append(os.path.join(NR, 'nuclearData'), '\nGEMSodiumLevelRef 2.2;\nGEMFlowFaceZone "gemCut";\nGEMReactivityMap ( (0.5 1e-4) (0 0) (-0.5 -1e-4) );\n')
    # the face zone is made by make_pk_variant_r63.sh (topoSet + setsToZones) after this script
elif variant.startswith('prof_'):
    # gh:#1930 round 66: ONE edit, `nuclearData.externalReactivityTimeProfile` replaced by the Function1 type named; every value is chosen so
    # that the profile is near the shipped 5.7671e-4 (57.67 pcm) at t = 100 and moves by 1e-5..1e-4 over the 30 s of the run
    L = '5.7671e-4'
    profiles = {
        'prof_constant': 'type constant;\n    value %s;' % L,
        'prof_uniform': 'type uniform;\n    value %s;' % L,
        'prof_zero': 'type zero;',
        'prof_one': 'type one;',
        'prof_linearRamp': 'type linearRamp;\n    start 42.329;\n    duration 1e5;',
        'prof_halfCosineRamp': 'type halfCosineRamp;\n    start -1430;\n    duration 1e5;',
        'prof_quarterCosineRamp': 'type quarterCosineRamp;\n    start -2062;\n    duration 1e5;',
        'prof_quarterSineRamp': 'type quarterSineRamp;\n    start 63.29;\n    duration 1e5;',
        'prof_cosine': 'type cosine;\n    frequency 0.05;\n    scale 1e-4;\n    level %s;' % L,
        'prof_square': 'type square;\n    frequency 0.05;\n    mark 1;\n    space 2;\n    scale 1e-4;\n    level %s;' % L,
        'prof_polynomial': 'type polynomial;\n    coeffs ( (4.5671e-4 0) (1e-6 1) (2e-9 2) );',
    }
    nd = os.path.join(NR, 'nuclearData')
    s = open(nd).read()
    s, n = re.subn(r'externalReactivityTimeProfile\s*\{.*?\n\}', 'externalReactivityTimeProfile\n{\n    ' + profiles[variant] + '\n}', s, count=1, flags=re.S)
    assert n == 1
    open(nd, 'w').write(s)
else:
    sys.exit('unknown variant ' + variant)
