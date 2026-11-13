"""Run only the end-to-end accident_case group of gen_triso_atops_reference.py
against whichever upstream checkout sits at ../upstream_source/TRISO-ATOPS, and
write the rows to ../out.csv (same format as the fixture)."""
import sys
from pathlib import Path
here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
import gen_triso_atops_reference as g

driver, driver_calc = g.load_upstream_driver()
g.gen_accident_case(driver, driver_calc)
lines = []
for fn, a, exp, cond in g.ROWS:
    lines.append(f"{fn},{';'.join(g.fmt(x) for x in a)},{g.fmt(exp)},{g.fmt(cond)}")
(here.parent / sys.argv[1]).write_text("\n".join(lines) + "\n")
print(len(lines), "rows")
