# gh:#1852 / #1854 round 30: keep the committed reference small. usage: subsample_coupled_deform_dump.py <full dump> <out.gz> <stride>
# Every `stride`-th item of every field (plus the last), exactly as dumped (17 digits); `S` state lines are copied whole.
# Line format: "G <call> <key> <ncomp> <nItems> <stride> idx:v1,v2,v3 ..."
import sys, gzip

src, dst, stride = sys.argv[1], sys.argv[2], int(sys.argv[3])
vector_names = {'points', 'oldPoints', 'reverseVector', 'disp', 'disp.bnd'}
with gzip.open(dst, 'wt') as o:
    for line in open(src):
        t = line.split(' ')
        if t[0] == 'F':
            call, key = t[1], t[2]
            nc = 3 if key.split(':')[-1] in vector_names else 1
            v = [x.strip() for x in t[3:]]
            n = len(v) // nc
            idx = sorted(set(list(range(0, n, stride)) + [n - 1]))
            o.write('G %s %s %d %d %d' % (call, key, nc, n, stride))
            for i in idx:
                o.write(' %d:%s' % (i, ','.join(v[i * nc:(i + 1) * nc])))
            o.write('\n')
        else:
            o.write(line)
