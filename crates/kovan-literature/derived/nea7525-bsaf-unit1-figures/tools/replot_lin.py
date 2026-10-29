"""Redraw digitised series from the CSVs alone, linear axes (Unit 1 figures).

A linear-axis sibling of replot.py (which is hard-wired to the log-rate,
20-day axes of Figs 5.1-5.4). Renders only the extracted numbers, never the
source figure, so the PNG can be published while the figure stays private.
Measurement series are drawn as markers, code series as lines (a gap longer
than GAP x-units is left as a gap, not bridged).

usage: replot_lin.py TITLE OUT.png XMAX XSTEP YMAX YSTEP XLABEL YLABEL  path|hex|label|line-or-marker ...
"""
import csv, os, subprocess, sys

W, H = 1500, 950
L, R, T, B = 150, 40, 60, 90
GAP = float(os.environ.get('GAP', '0.25'))

def main():
    title, dst = sys.argv[1], sys.argv[2]
    xmax, xstep, ymax, ystep = map(float, sys.argv[3:7])
    xlabel, ylabel = sys.argv[7], sys.argv[8]
    specs = sys.argv[9:]
    xp = lambda x: L + (x / xmax) * (W - L - R)
    yp = lambda y: H - B - (y / ymax) * (H - T - B)
    cmd = ['convert', '-size', f'{W}x{H}', 'xc:white', '-font', 'DejaVu-Sans',
           '-stroke', '#dddddd', '-strokewidth', '1']
    n = int(round(ymax / ystep))
    for k in range(n + 1):
        cmd += ['-draw', f'line {L},{yp(k*ystep):.1f} {W-R},{yp(k*ystep):.1f}']
    m = int(round(xmax / xstep))
    for k in range(m + 1):
        cmd += ['-draw', f'line {xp(k*xstep):.1f},{T} {xp(k*xstep):.1f},{H-B}']
    cmd += ['-stroke', 'black', '-strokewidth', '2',
            '-draw', f'line {L},{T} {L},{H-B}', '-draw', f'line {L},{H-B} {W-R},{H-B}',
            '-stroke', 'none', '-fill', 'black', '-pointsize', '20']
    for k in range(n + 1):
        v = k * ystep
        cmd += ['-draw', f'text 70,{yp(v)+7:.1f} "{v:g}"']
    for k in range(m + 1):
        v = k * xstep
        cmd += ['-draw', f'text {xp(v)-8:.1f},{H-B+28} "{v:g}"']
    cmd += ['-pointsize', '22', '-draw', f'text {L},{T-22} "{title}"',
            '-pointsize', '20', '-draw', f'text {W/2-120},{H-22} "{xlabel}"',
            '-draw', f'text 10,{T-22} "{ylabel}"']
    ly = T + 14
    for spec in specs:
        path, colour, label, style = spec.split('|')
        if not os.path.exists(path): continue
        pts = []
        for row in csv.reader(open(path)):
            try: pts.append((float(row[0]), float(row[1])))
            except (ValueError, IndexError): pass
        pts.sort()
        if style == 'marker':
            cmd += ['-stroke', 'black', '-strokewidth', '1', '-fill', '#' + colour]
            for x, y in pts:
                cmd += ['-draw', f'rectangle {xp(x)-4:.1f},{yp(y)-4:.1f} {xp(x)+4:.1f},{yp(y)+4:.1f}']
        else:
            cmd += ['-stroke', '#' + colour, '-strokewidth', '2', '-fill', 'none']
            seg = []
            for i, (x, y) in enumerate(pts):
                if i and x - pts[i-1][0] > GAP:
                    if len(seg) > 1: cmd += ['-draw', 'polyline ' + ' '.join(seg)]
                    seg = []
                seg.append(f'{xp(x):.1f},{yp(y):.1f}')
            if len(seg) > 1: cmd += ['-draw', 'polyline ' + ' '.join(seg)]
        cmd += ['-stroke', '#' + colour, '-strokewidth', '4', '-fill', '#' + colour,
                '-draw', f'line {W-R-200},{ly} {W-R-160},{ly}',
                '-stroke', 'none', '-fill', 'black', '-pointsize', '18',
                '-draw', f'text {W-R-150},{ly+6} "{label}"']
        ly += 26
    cmd.append(dst)
    subprocess.run(cmd, check=True)
    print('wrote', dst)

main()
