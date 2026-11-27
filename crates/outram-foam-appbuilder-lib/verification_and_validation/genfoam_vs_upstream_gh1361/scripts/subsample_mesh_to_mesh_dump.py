# gh:#651 round 28: keep the committed reference small. usage: subsample_mesh_to_mesh_dump.py <full dump> <out.gz> <stride>
# From the full meshToMeshDump output keeps, exactly as dumped (17 digits):
#   * every `V` line (the total overlap volumes, whole);
#   * every `L` (addressing + weights) line of every `stride`-th cell, the last cell, and EVERY cell whose entry count differs
#     from the most common count of its list (the 18 uncovered thermoMechanical cells; the cells at the edge of the moved mesh);
#   * every `M` (mapped field) line as "S <obj> <name> <ncomp> <nItems> idx:v1[,v2,v3] ..." for every `stride`-th item and the last.
import sys, gzip, collections

src, dst, stride = sys.argv[1], sys.argv[2], int(sys.argv[3])
ncomp = {'tgtToSrcScalar': 1, 'tgtToSrcVector': 3, 'srcToTgtScalar': 1}

# pass 1: modal entry count of each list
modal = {}
counts = collections.defaultdict(collections.Counter)
nl = collections.Counter()
for line in open(src):
    if line.startswith('L '):
        t = line.split(' ', 5)
        counts[(t[1], t[2])][int(t[4])] += 1
        nl[(t[1], t[2])] += 1
for k, c in counts.items():
    # a count counts as "the rule with rare exceptions" only if >= 99 % of the cells have it
    m, nm = c.most_common(1)[0]
    modal[k] = m if nm >= 0.99 * nl[k] else None

with gzip.open(dst, 'wt') as o:
    for line in open(src):
        if line.startswith('V '):
            o.write(line)
        elif line.startswith('L '):
            t = line.split(' ', 5)
            key = (t[1], t[2])
            cell, n = int(t[3]), int(t[4])
            if cell % stride == 0 or cell == nl[key] - 1 or (modal[key] is not None and n != modal[key]):
                o.write(line)
        elif line.startswith('M '):
            t = line.rstrip('\n').split(' ')
            obj, name = t[1], t[2]
            nc = ncomp[name]
            v = t[3:]
            n = len(v) // nc
            idx = sorted(set(list(range(0, n, stride)) + [n - 1]))
            o.write('S %s %s %d %d' % (obj, name, nc, n))
            for i in idx:
                o.write(' %d:%s' % (i, ','.join(v[i * nc:(i + 1) * nc])))
            o.write('\n')
