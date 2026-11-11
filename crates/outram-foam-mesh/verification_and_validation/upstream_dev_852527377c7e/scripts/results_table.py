"""Markdown results table from a compare_polymesh log (results/*_compare_*.txt).
usage: python3 -I results_table.py LOG > TABLE.md"""
import re, sys
rows = []; cur = None
for l in open(sys.argv[1]):
    l = l.rstrip('\n')
    if l.startswith('=== '):
        cur = {'name': re.sub(r' \(centres \w+\)', '', l[4:])}; rows.append(cur)
    m = re.match(r'points: ours (\d+) upstream (\d+); bit-identical (\d+); worst \|diff\| (\S+)', l)
    if m: cur.update(n=int(m[1]), bi=int(m[3]), w=m[4])
    m = re.match(r'faces: ours (\d+) upstream (\d+); identical loops (\d+)', l)
    if m: cur.update(f=int(m[1]), fi=int(m[3]))
    m = re.match(r'owner: ours (\d+) upstream (\d+); equal (\d+)', l)
    if m: cur.update(o=int(m[1]), oe=int(m[3]))
    m = re.match(r'neighbour: ours (\d+) upstream (\d+); equal (\d+)', l)
    if m: cur.update(nb=int(m[1]), ne=int(m[3]))
    if l.strip().startswith('equal: '): cur['b'] = l.strip().endswith('true')
print('| case | points bit-identical / total | worst abs diff | topology identical? (faces / owner / neighbour / patches) |')
print('|---|---|---|---|')
for r in rows:
    topo = r['fi'] == r['f'] and r['oe'] == r['o'] and r['ne'] == r['nb'] and r['b']
    print(f"| {r['name']} | {r['bi']} / {r['n']} | {float(r['w']):.3g} | {'yes' if topo else 'NO'} ({r['fi']}/{r['f']}, {r['oe']}/{r['o']}, {r['ne']}/{r['nb']}, {'equal' if r['b'] else 'DIFFER'}) |")
