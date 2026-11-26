import sys, collections
CALLS = [int(x) for x in sys.argv[1].split(',')]

def kv(line):
    return {x.split('=')[0]: float(x.split('=')[1]) for x in line.split()[1:] if '=' in x}

# upstream: STAGE_Sx lines in order; a new EEqn call begins at each STAGE_S0 block (10 cells)
up = collections.defaultdict(dict)   # (call, stage) -> cell -> d
call = 0
for line in open('up_psbt.txt'):
    if line.startswith('STAGE_'):
        st = line.split()[0][6:]
        d = kv(line)
        if st == 'S0' and int(d['cell']) == 20:
            call += 1
        up[(call, st)][int(d['cell'])] = d
port = collections.defaultdict(dict)
key = None
n = 0
for line in open('port_psbt2.txt'):
    tag = line.split()[0]
    if tag.startswith('PORT_S'):
        st = tag[5:]
        d = kv(line)
        k = (d['t'], d['corr'])
        if st == 'S0' and int(d['cell']) == 20:
            n += 1
        port[(n, st)][int(d['cell'])] = d
print('calls', call, n)
for c in CALLS:
    row = []
    for st in ('S0', 'S1', 'S2', 'S3'):
        for q in ('diag', 'source'):
            w = 0.0
            wc = -1
            for cell, d in up[(c, st)].items():
                p = port[(c, st)].get(cell)
                if p is None:
                    continue
                a = d[q]
                b = p[q]
                r = abs(a - b) / abs(a) if a != 0 else abs(b)
                if r > w:
                    w, wc = r, cell
            row.append('%s.%s=%.1e@%d' % (st, q, w, wc))
    print('call', c, ' '.join(row))
