import sys, collections
UP = 'up_psbt.txt'; PORT = sys.argv[1]
MAXCALL = int(sys.argv[2]) if len(sys.argv) > 2 else 6

def parse(line):
    w = line.split()
    d = {}
    for x in w[1:]:
        if '=' in x:
            k, v = x.split('=', 1)
            d[k] = float(v)
    return w[0], d

up = collections.defaultdict(dict)   # call -> cell -> dict
upf = collections.defaultdict(dict)
for line in open(UP):
    if line.startswith('EQN1F '):
        tag, d = parse(line)
        upf[int(d['call'])][int(d['cell'])] = d
    elif line.startswith('EQN1 '):
        tag, d = parse(line)
        up[int(d['call'])][int(d['cell'])] = d
# port: calls in file order; a new call when (t, corr) changes
pc = collections.defaultdict(dict)
pf = collections.defaultdict(dict)
key = None
n = 0
for line in open(PORT):
    if line.startswith('PORTE '):
        tag, d = parse(line)
        k = (d['t'], d['corr'])
        if k != key:
            key = k
            n += 1
        pc[n][int(d['cell'])] = d
    elif line.startswith('PORTEF '):
        tag, d = parse(line)
        pf[n][int(d['cell'])] = d
print('calls up', len(up), 'port', len(pc))
for call in range(MAXCALL - 8, MAXCALL + 1):
    row = []
    for q in ('diag', 'source', 'he', 'T', 'alpha2', 'norm2', 'T2', 'he2', 'iT', 'dmdt', 'rho2'):
        w = 0.0
        wc = -1
        for c, d in up[call].items():
            if c in pc[call] and c not in (0, 74):
                a = d[q]
                b = pc[call][c][q]
                r = abs(a - b) / abs(a) if a != 0 else abs(b)
                if r > w:
                    w = r
                    wc = c
        row.append('%s=%.1e@%d' % (q, w, wc))
    for q in ('upper', 'lower'):
        w = 0.0
        wc = -1
        for c, d in upf[call].items():
            if c in pf[call]:
                a = d[q]
                b = pf[call][c][q]
                r = abs(a - b) / abs(a) if a != 0 else abs(b)
                if r > w:
                    w = r
                    wc = c
        row.append('%s=%.1e@%d' % (q, w, wc))
    print('call', call, ' '.join(row))
