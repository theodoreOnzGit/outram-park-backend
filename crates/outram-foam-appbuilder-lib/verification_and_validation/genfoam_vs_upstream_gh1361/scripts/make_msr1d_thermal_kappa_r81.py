# gh:#1962 (README round 81): the `kappaMatrix` VARIANTS of 1D_thermalMSR_pointKinetics (not shipped: the shipped case has no `kappaMatrix`
# key, so `kappaMatrix = 0` and upstream's `fvc::laplacian(Tmatrix)` is evaluated on an all-zero field and never used). Each variant is
# applied, as text, to BOTH the real run's case and the port's staged copy (`constant/fluidRegion/phaseProperties` only):
#   core: the core's `lumpedNuclearStructure` gets `nodeMatrix 0; kappaMatrix 30;` (graphite 30 W/m/K, the tutorial's own comment). The
#         core (cells 107-266) does not touch the cyclic seam (cells 0 and 639), so the coupled-patch branch contributes exactly 0 here.
#   seam: `core` as above AND a second `lumpedNuclearStructure` region on `ColdLeg` (cells 0-106 and 587-639, the zone that holds BOTH
#         seam cells): `volumeFraction 0.1; Dh 0.02;`, `volumetricArea 40`, `powerDensity 1e6`, the core's node data, `kappaMatrix 30`, and a
#         `constant` heat transfer of 1e4 W/m2/K on `ColdLeg`; this is the variant whose seam faces carry a non-zero `snGrad(Tmatrix)`.
# usage: python3 -I make_msr1d_thermal_kappa_r81.py <phaseProperties> <core|seam>
import sys

path, kind = sys.argv[1], sys.argv[2]
s = open(path).read()
anchor = "            nodeClad            0;"
assert s.count(anchor) >= 1
i = s.index(anchor)
j = s.index("\n", i)
s = s[: j + 1] + "            nodeMatrix          0;\n            kappaMatrix         30;\n" + s[j + 1 :]
if kind == "seam":
    hx = '    "hx"\n    {\n        volumeFraction      0.6;'
    assert s.count(hx) == 1
    cold = (
        '    "ColdLeg"\n    {\n        volumeFraction      0.1;\n        Dh                  0.02;\n\n        powerModel\n        {\n'
        "            type                lumpedNuclearStructure;\n            volumetricArea      40;\n            powerDensity        1e6;\n"
        "            nodesNumber         1;\n            nodeFuel            0;\n            nodeClad            0;\n            nodeMatrix          0;\n"
        "            kappaMatrix         30;\n            heatConductances    (450000 225000);\n            rhoCp               (5357692);\n"
        "            volumeFractions     (1);\n            powerFractions      (1);\n            T0                  900;\n        }\n    }\n"
    )
    s = s.replace(hx, cold + hx)
    ht = '        "hx"\n        {\n            type    constant;'
    assert s.count(ht) == 1
    s = s.replace(ht, '        "ColdLeg"\n        {\n            type    constant;\n            value   1e+4;\n        }\n' + ht)
elif kind != "core":
    raise SystemExit("kind: core|seam")
open(path, "w").write(s)
print("variant", kind)
