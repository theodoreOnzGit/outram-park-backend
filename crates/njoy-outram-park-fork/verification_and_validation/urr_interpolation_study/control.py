exec(open('/tmp/dens/compare.py').read().split('base = mf3')[0])
base = mf3("/tmp/u234/tape22", 9225, 18)
dens = mf3("/tmp/dens/run/tape22", 9225, 18)
bd = {round(e,6): v for e,v in base}
dd = {round(e,6): v for e,v in dens}
# CONTROL: at the ORIGINAL parameter energies NJOY evaluates in both runs,
# so densification must change nothing there.
params=[1.5e3,2.5e3,3.5e3,5.0e3,8.0e3,1.5e4,2.5e4,4.0e4,6.0e4]
print(f"{'E (eV)':>12} {'NJOY base':>13} {'NJOY x4':>13} {'rel diff':>11}")
worst=0.0
for e in params:
    k=round(e,6); b=bd.get(k); d=dd.get(k)
    if b is None or d is None:
        print(f"{e:12.5e} {'absent':>13} {'absent':>13}"); continue
    r=abs(d-b)/abs(b); worst=max(worst,r)
    print(f"{e:12.5e} {b:13.6e} {d:13.6e} {r:11.2e}")
print(f"\nworst change at an original parameter energy: {worst:.2e}")
# also: how many points do the two grids share, and do they agree there?
shared=set(bd)&set(dd)
devs=[abs(dd[k]-bd[k])/abs(bd[k]) for k in shared if bd[k]!=0]
devs.sort()
print(f"shared grid points: {len(shared)}; median rel diff {devs[len(devs)//2]:.2e}; "
      f"95th pct {devs[int(0.95*len(devs))]:.2e}; max {devs[-1]:.2e}")
