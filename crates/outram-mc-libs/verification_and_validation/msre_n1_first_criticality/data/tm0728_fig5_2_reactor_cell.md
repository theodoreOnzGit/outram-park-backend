# ORNL-TM-0728 Fig. 5.2, Reactor Cell Elevation: dimensioned data

**Source.** R. C. Robertson, *MSRE Design and Operations Report Part I:
Description of Reactor Design*, ORNL-TM-0728 (1965), Fig. 5.2 (printed
p. 65, PDF p. 98), ORNL DWG 64-8811. A US government report in the public
domain, mirrored at moltensalt.org. Read 2026-11-12 for gh:#1128.

**What the figure is.** A reduced engineering drawing, not a schematic. It
has no dimension lines, but it carries elevation callouts (`EL.` in
feet-inches above sea level) and is drawn close to scale. Two checks:
- with the vessel's 59-1/8 in OD (Table 5.2) as the scale, the shield
  cylinder measures 154 in against its stated 151 in (+2 %);
- the scaled distributor height, 93 in above the cylinder bottom, equals
  the callout's 832'-9" less 825'-0".

`images/tm0728_fig5_2_vessel_in_shield.png` is the vessel and shield
region at 200 dpi.

**Method.** The page was rendered with `pdftoppm` (110 to 1200 dpi). Callouts
were read by eye. Positions without a callout were scaled from the vessel
OD. **The scan is coarse.** Both callouts near the vessel have one blurred
digit even at 1200 dpi, so each is reconstructed and its check stated:
- **shield top:** read as `8?1-7`. Reconstructed as **837'-7"**, from the
  platform at 823'-6" (TM-0728 p. 32) plus the 18 in base and the 151 in
  cylinder (§5.3.10.1).
- **inlet distributor:** read as `8?2-9`. Taken as **832'-9"**, because only
  a 3 agrees with the scaled 93 in.

**Data:** `tm0728_fig5_2_reactor_cell.csv`. Each row says whether it is text,
callout, reconstructed or scaled, with its uncertainty. Scaled rows carry
±3 in (2 % of the 151 in cylinder).

**What it settles for the N-1 shield model (`Shield::Tm0728Cylinder`):**
- **Elevation.** The vessel spans about 23 to 114 in above the shield
  cylinder's bottom (151 in tall). The vessel's mid-height, about 68 in,
  sits about 7 ± 4 in below the shield's. The model centres them.
- **Below the vessel:** the 18 in base (I-beams between 3/4 in 304 plates)
  and 6 in of insulation, about 23 in below it. The model has void there.
- **Above the vessel:** the 16 in cover (a 304 box holding four 1 in
  carbon-steel plates per segment, with water between) at 135 to 151 in.
  The model leaves the cylinder open there.

**Not taken from it:** the graphite's position inside the vessel. Fig. 5.2
does not show it. The model takes it from TM-0728 §5.3 and TM-0730
Table 3.1.
