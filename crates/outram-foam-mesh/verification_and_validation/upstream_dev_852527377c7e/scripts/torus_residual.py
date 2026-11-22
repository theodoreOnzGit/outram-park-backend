# Residual of dev-binary blockMesh points against the torus (centre (6,0,0), axis y, R=2, r=1),
# over the boundary-face points (needs points, faces, boundary of a polyMesh dir).
import re, sys, math
d = sys.argv[1]
def body(f):
    t = open(f'{d}/{f}').read()
    t = t[t.index('}')+1:]
    return t
pts = [tuple(map(float, m)) for m in re.findall(r'\(([-\d.e+]+) ([-\d.e+]+) ([-\d.e+]+)\)', body('points'))]
faces = [list(map(int, m.split())) for m in re.findall(r'\d+\(([\d ]+)\)', body('faces'))]
nint = int(re.search(r'^(\d+)\s*\(', body('neighbour'), re.M).group(1))
bp = sorted({v for f in faces[nint:] for v in f})
res = lambda p: abs(math.sqrt((math.hypot(p[0]-6, p[2]) - 2)**2 + p[1]**2) - 1)
r = [res(pts[i]) for i in bp]
near = [x for x in r if x < 1e-3]
print(f'{d.split("/")[-3]}: boundary points {len(bp)}; off torus >1e-9: {sum(x>1e-9 for x in r)}; worst near-torus (<1e-3) residual {max(near):.12e}')
