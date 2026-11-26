import math as m, runpy
ns=runpy.run_path(__file__.replace('fm.py','ae.py'))
A=ns['A'];kB=ns['kB']
for d in (1e-10,1e-9,3e-10):
    v=m.sqrt(8*kB*A['T']/(m.pi*A['rp']*m.pi*d**3/6)); vij=m.sqrt(2)*v
    fm=m.pi/4*(2*d)**2*vij; print("d",d,"beta %.6e fm %.6e ratio %.6f"%(ns['brown'](A,d,d),fm,ns['brown'](A,d,d)/fm))
