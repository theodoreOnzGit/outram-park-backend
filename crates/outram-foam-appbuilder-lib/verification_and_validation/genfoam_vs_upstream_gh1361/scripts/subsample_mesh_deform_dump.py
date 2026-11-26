# gh:#1784 round 26: keep the committed reference small. usage: subsample_mesh_deform_dump.py <full dump> <out.gz> <stride>
# Every `stride`-th item of every field (plus the last), exactly as dumped (17 digits); `P` and `I` lines are copied whole.
# Line format: "S <step> <region:name> <ncomp> <nItems> <stride> idx:v1,v2,v3 ..."
import sys, gzip

src, dst, stride = sys.argv[1], sys.argv[2], int(sys.argv[3])
ncomp = {'pointDisp': 3, 'points': 3, 'Cf': 3, 'Sf': 3, 'C': 3, 'V': 1}
with gzip.open(dst, 'wt') as o:
    for line in open(src):
        t = line.split(' ')
        if t[0] == 'F':
            step, key = t[1], t[2]
            nc = ncomp[key.split(':')[1]]
            v = [x.strip() for x in t[3:]]
            n = len(v) // nc
            idx = sorted(set(list(range(0, n, stride)) + [n - 1]))
            o.write('S %s %s %d %d %d' % (step, key, nc, n, stride))
            for i in idx:
                o.write(' %d:%s' % (i, ','.join(v[i * nc:(i + 1) * nc])))
            o.write('\n')
        else:
            o.write(line)
