import sys, collections
CALL = int(sys.argv[1])
CELLS = [int(x) for x in sys.argv[2].split(',')]

def kv(line):
    return {x.split('=')[0]: float(x.split('=')[1]) for x in line.split()[1:] if '=' in x}

# upstream: PC lines are emitted inside correctModels (before that iteration's EEqns). Assign the EEqn call
# index = number of EQN1 cell-0 lines seen so far + 1.
up = {}
eq = 0
for line in open('up_psbt.txt'):
    if line.startswith('EQN1 '):
        d = kv(line)
        if int(d['cell']) == 0:
            eq += 1
    elif line.startswith('PC '):
        d = kv(line)
        up[(eq + 1, int(d['cell']))] = d
port = {}
key = None
n = 0
for line in open('port_psbt2.txt'):
    tag = line.split()[0]
    if tag == 'PORTE':
        d = kv(line)
        if int(d['cell']) == 0:
            n += 1
    elif tag == 'PORT_PC':
        d = kv(line)
        port[(n + 1, int(d['cell']))] = d
names = ['dmdt', 'fw', 'dmdtW', 'iA', 'L', 'htc1', 'htc2', 'q1', 'q2', 'f', 'dmdtI0', 'c1', 'c2', 'he1', 'he2', 'iT', 'T1', 'T2']
for cell in CELLS:
    u = up.get((CALL, cell))
    p = port.get((CALL, cell))
    if u is None or p is None:
        print('cell', cell, 'missing', u is None, p is None)
        continue
    print('cell', cell)
    for q in names:
        a, b = u[q], p[q]
        r = abs(a - b) / abs(a) if a != 0 else abs(b)
        print('  %-7s up=%.12e port=%.12e rel=%.2e' % (q, a, b, r))
