# INOR-8 (ORNL-TM-0728 Table 2.2): 0.317 lb/in3; Ni 71, Mo 16.5, Cr 7, Fe 4 wt % (minor 1.5 % omitted).
NA=0.6022140857
rho=0.317*27.679904710   # lb/in3 -> g/cm3
el={'Ni':(0.71,58.6934,{'Ni58':0.680769,'Ni60':0.262231,'Ni61':0.011399,'Ni62':0.036345,'Ni64':0.009256}),
    'Cr':(0.07,51.9961,{'Cr50':0.04345,'Cr52':0.83789,'Cr53':0.09501,'Cr54':0.02365}),
    'Fe':(0.04,55.845,{'Fe54':0.05845,'Fe56':0.91754,'Fe57':0.02119,'Fe58':0.00282}),
    'Mo':(0.165,95.95,None)}
print(f"rho {rho:.5f}")
for e,(w,A,iso) in el.items():
    n=rho*w/A*NA
    if iso:
        for k,f in iso.items(): print(f"    (\"{k}\", {n*f:.6e}),")
    else:
        print(f"    Mo element {n:.6e}; Mo-95 at equal 2200 m/s absorption (x 2.48/13.4): {n*2.48/13.4:.6e}")

# Natural Mo (arm C, `--mo natural`, 2026-11-09): IUPAC 2013 isotopic abundances,
# atom fraction, on the ENDF/B-VIII.0 release tapes added to reference-data/ace (ba58c2e).
MO_NAT={'Mo92':0.1453,'Mo94':0.0915,'Mo95':0.1584,'Mo96':0.1667,'Mo97':0.0960,'Mo98':0.2439,'Mo100':0.0982}
n_mo=rho*0.165/95.95*NA
for k,f in MO_NAT.items(): print(f"    (\"{k}\", {n_mo*f:.6e}),")
