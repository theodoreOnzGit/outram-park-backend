exec(open('/tmp/dens/compare.py').read().split('base = mf3')[0])
runs = {"base":"/tmp/u234/tape22", "x2":"/tmp/dens/run2/tape22",
        "x4":"/tmp/dens/run/tape22", "x8":"/tmp/dens/run8/tape22",
        "x16":"/tmp/dens/run16/tape22"}
D = {k: {round(e,6): v for e,v in mf3(p, 9225, 18)} for k,p in runs.items()}
ours = {7.0e3:1.229451e-2, 4.368748e4:1.476871e-2, 4.518e4:1.438642e-2,
        5.25e4:1.272450e-2, 7.0e4:1.436331e-2, 8.0e4:1.679084e-2, 9.0e4:1.875089e-2}
cols=list(runs)
print(f"{'E (eV)':>12} " + " ".join(f"{c:>10}" for c in cols) + "    (relative deviation from our evaluated value)")
for e in sorted(ours):
    k=round(e,6); o=ours[e]
    row=[]
    for c in cols:
        v=D[c].get(k)
        row.append("        -" if v is None else f"{(v-o)/o:10.2e}")
    print(f"{e:12.5e} " + " ".join(row))
# control at parameter energies across all runs
print("\nCONTROL -- deviation from BASELINE at original parameter energies:")
params=[2.5e3,3.5e3,5.0e3,8.0e3,1.5e4,2.5e4,4.0e4,6.0e4]
for c in cols[1:]:
    w=max(abs(D[c][round(e,6)]-D["base"][round(e,6)])/abs(D["base"][round(e,6)])
          for e in params if round(e,6) in D[c] and round(e,6) in D["base"])
    print(f"  {c:>5}: worst {w:.2e}")
