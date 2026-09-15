import sys
def pf(t):
    t=t.strip()
    if not t: return 0.0
    for i in range(1,len(t)):
        if t[i] in "+-" and t[i-1] not in "eE": return float(t[:i]+"e"+t[i:])
    return float(t)

def mf3(path, mat, mt):
    rows=[l for l in open(path).read().split("\n")
          if len(l)>=75 and l[66:70].strip()==str(mat) and l[70:72].strip()=="3" and l[72:75].strip()==str(mt)]
    if not rows: return []
    # CONT, then TAB1 head + NR interp pairs + NP data pairs
    head=rows[1]
    nr=int(head[44:55]); np_=int(head[55:66])
    ni=(2*nr+5)//6
    vals=[]
    for l in rows[2+ni:]:
        for k in range(6):
            f=l[k*11:(k+1)*11]
            if f.strip(): vals.append(pf(f))
    return [(vals[i],vals[i+1]) for i in range(0,min(len(vals),2*np_),2)]

base = mf3("/tmp/u234/tape22", 9225, 18)
dens = mf3("/tmp/dens/run/tape22", 9225, 18)
print(f"baseline points: {len(base)}, densified points: {len(dens)}")
bd = {round(e,6): v for e,v in base}
dd = {round(e,6): v for e,v in dens}

# the 8 energies where our kernel disagreed with baseline NJOY
targets = [7.0e3, 4.368748e4, 4.368749e4, 4.518e4, 5.25e4, 7.0e4, 8.0e4, 9.0e4]
ours = {7.0e3:1.229451e-2, 4.368748e4:1.476871e-2, 4.368749e4:1.476871e-2,
        4.518e4:1.438642e-2, 5.25e4:1.272450e-2, 7.0e4:1.436331e-2,
        8.0e4:1.679084e-2, 9.0e4:1.875089e-2}
print(f"{'E (eV)':>12} {'NJOY base':>12} {'NJOY x4':>12} {'ours':>12}   {'base-ours':>10} {'x4-ours':>10}")
for e in targets:
    k=round(e,6)
    b=bd.get(k); d=dd.get(k); o=ours[e]
    if b is None or d is None:
        print(f"{e:12.5e} {'-' if b is None else f'{b:.6e}':>12} {'-' if d is None else f'{d:.6e}':>12} {o:12.6e}")
        continue
    print(f"{e:12.5e} {b:12.6e} {d:12.6e} {o:12.6e}   {(b-o)/o:10.2e} {(d-o)/o:10.2e}")
