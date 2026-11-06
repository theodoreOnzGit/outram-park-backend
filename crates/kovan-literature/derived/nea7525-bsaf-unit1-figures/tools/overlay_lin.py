"""Re-project digitised points onto the source figure (linear axes).

Linear-axis sibling of overlay.py. The output reproduces the copyrighted
figure, so it is written ONLY to the private repository, never here.

usage: overlay_lin.py SRC DST XPX0 XVAL0 XPX1 XVAL1 YPX0 YVAL0 YPX1 YVAL1 csv:hex ...
"""
import csv, os, subprocess, sys

src, dst = sys.argv[1], sys.argv[2]
x0p, x0v, x1p, x1v, y0p, y0v, y1p, y1v = map(float, sys.argv[3:11])
cmd = ['convert', src, '-fill', 'white', '-colorize', '55%']
for spec in sys.argv[11:]:
    path, colour = spec.rsplit(':', 1)
    if not os.path.exists(path): continue
    pts = []
    for row in csv.reader(open(path)):
        try: x, y = float(row[0]), float(row[1])
        except (ValueError, IndexError): continue
        pts.append((x0p + (x - x0v) * (x1p - x0p) / (x1v - x0v), y0p + (y - y0v) * (y1p - y0p) / (y1v - y0v)))
    r = 3.0 if 'measurement' in path else 1.3
    cmd += ['-stroke', 'none', '-fill', '#' + colour]
    if pts:
        cmd += ['-draw', ' '.join(f'rectangle {a-r:.1f},{b-r:.1f} {a+r:.1f},{b+r:.1f}' for a, b in pts)]
cmd.append(dst)
subprocess.run(cmd, check=True)
print('wrote', dst)
