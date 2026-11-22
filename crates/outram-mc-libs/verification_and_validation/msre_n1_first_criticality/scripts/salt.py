# MSRE salt at first criticality (ORNL-4233 sec. 2) -> atom densities.
NA=0.6022140857  # 1e24/mol, so g/cm3 / (g/mol) * NA = atoms/b-cm
m={'Li6':6.0151228874,'Li7':7.0160034366,'Be9':9.012183065,'F19':18.998403163,
   'U235':235.0439299,'U238':238.0507882,
   'Zr90':89.9046977,'Zr91':90.9056396,'Zr92':91.9050347,'Zr94':93.9063108,'Zr96':95.9082714}
zr_ab={'Zr90':0.5145,'Zr91':0.1122,'Zr92':0.1715,'Zr94':0.1738,'Zr96':0.0280}
li7=0.9999
mLi=li7*m['Li7']+(1-li7)*m['Li6']; mZr=sum(zr_ab[k]*m[k] for k in zr_ab)
mLiF=mLi+m['F19']; mBeF2=m['Be9']+2*m['F19']; mZrF4=mZr+4*m['F19']
assay=0.002   # depleted-U assay, NOT given in ORNL-4233; ablated below
def comp(u235_kg=69.6, wt_book=0.01414, flush_kg=20.0, assay=assay):
    salt_kg=u235_kg/wt_book
    dep_U=150.0; enr_U=(u235_kg-dep_U*assay)/0.93
    U238_kg=dep_U*(1-assay)+enr_U*0.07          # U-234/236 of the 93 % lot counted as U-238
    molU235=u235_kg*1e3/m['U235']; molU238=U238_kg*1e3/m['U238']; molU=molU235+molU238
    mUF4=(u235_kg*1e3+U238_kg*1e3)+molU*4*m['F19']
    molLiF_eut=molU*73/27; mLiF_eut=molLiF_eut*mLiF
    carrier_g=salt_kg*1e3-mUF4-mLiF_eut
    w=0.65*mLiF+0.30*mBeF2+0.05*mZrF4; molc=carrier_g/w
    mol={'LiF':molLiF_eut+0.65*molc,'BeF2':0.30*molc,'ZrF4':0.05*molc}
    fl=flush_kg*1e3/(0.66*mLiF+0.34*mBeF2); mol['LiF']+=0.66*fl; mol['BeF2']+=0.34*fl
    tot_g=salt_kg*1e3+flush_kg*1e3
    atoms={'Li7':mol['LiF']*li7,'Li6':mol['LiF']*(1-li7),'Be9':mol['BeF2'],
           'U235':molU235,'U238':molU238,
           'F19':mol['LiF']+2*mol['BeF2']+4*mol['ZrF4']+4*molU}
    for k in zr_ab: atoms[k]=mol['ZrF4']*zr_ab[k]
    moltot=mol['LiF']+mol['BeF2']+mol['ZrF4']+molU
    return atoms,tot_g,mol,molU,moltot,u235_kg*1e3/tot_g
rho=145.3*0.016018463  # lb/ft3 -> g/cm3 (ORNL-4233 p.10, 1181 F)
atoms,tot_g,mol,molU,moltot,w235=comp()
print(f"density {rho:.5f} g/cc; 235U wt% (flush-corrected) {100*w235:.4f}; 235U g/L {w235*rho*1e3:.3f}")
print("mol %:", {k:round(100*v/moltot,3) for k,v in mol.items()}, "UF4", round(100*molU/moltot,3))
for k,v in atoms.items(): print(f"    (\"{k}\", {v/tot_g*rho*NA:.6e}),")
a2,_,_,_,_,_=comp(assay=0.007)
print("U238 at assay 0.2% vs 0.7%:", atoms['U238']/tot_g*rho*NA, a2['U238']/tot_g*rho*NA)
