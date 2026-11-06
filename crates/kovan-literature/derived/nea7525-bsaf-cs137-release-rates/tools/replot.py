"""Redraw the digitised series from the CSVs alone.

Deliberately NOT an overlay on the source figure: this renders only the
extracted numbers, so it can be published beside the citation while the
copyrighted figure stays in the private repository. Put it next to the
printed figure to check the digitisation by eye.
"""
import csv, math, os, subprocess, sys

W, H = 1500, 950
L, R, T, B = 150, 40, 60, 90
YLO, YHI = 1e7, 1e15

def xp(x): return L + (x / 20.0) * (W - L - R)
def yp(y): return T + (math.log10(YHI / y) / 8.0) * (H - T - B)

def load(path):
    pts = []
    for row in csv.reader(open(path)):
        try:
            x, y = float(row[0]), float(row[1])
        except (ValueError, IndexError):
            continue
        if YLO <= y <= YHI:
            pts.append((x, y))
    pts.sort()
    return pts

def main():
    title, dst = sys.argv[1], sys.argv[2]
    specs = sys.argv[3:]
    cmd = ['convert', '-size', f'{W}x{H}', 'xc:white', '-font', 'DejaVu-Sans']
    cmd += ['-stroke', '#cccccc', '-strokewidth', '1']
    for d in range(9):
        y = T + d * (H - T - B) / 8.0
        cmd += ['-draw', f'line {L},{y} {W-R},{y}']
    for t in range(0, 21, 2):
        cmd += ['-draw', f'line {xp(t)},{T} {xp(t)},{H-B}']
    cmd += ['-stroke', 'black', '-strokewidth', '2',
            '-draw', f'line {L},{T} {L},{H-B}', '-draw', f'line {L},{H-B} {W-R},{H-B}']
    cmd += ['-stroke', 'none', '-fill', 'black', '-pointsize', '20']
    for d in range(9):
        y = T + d * (H - T - B) / 8.0
        cmd += ['-draw', f'text 60,{y+7} "1.E+{15-d:02d}"']
    for t in range(0, 21, 2):
        cmd += ['-draw', f'text {xp(t)-8},{H-B+28} "{t}"']
    cmd += ['-pointsize', '24', '-draw', f'text {L},{T-22} "{title}"']
    cmd += ['-pointsize', '20', '-draw', f'text {W/2-90},{H-22} "Elapsed time (day)"']
    ly = T + 14
    for spec in specs:
        path, colour, label = spec.split('|')
        if not os.path.exists(path):
            continue
        pts = load(path)
        cmd += ['-stroke', '#' + colour, '-strokewidth', '2', '-fill', 'none']
        seg = []
        for i, (x, y) in enumerate(pts):
            if i and x - pts[i-1][0] > 0.15:
                if len(seg) > 1:
                    cmd += ['-draw', 'polyline ' + ' '.join(seg)]
                seg = []
            seg.append(f'{xp(x):.1f},{yp(y):.1f}')
        if len(seg) > 1:
            cmd += ['-draw', 'polyline ' + ' '.join(seg)]
        cmd += ['-stroke', '#' + colour, '-strokewidth', '3',
                '-draw', f'line {W-R-260},{ly} {W-R-210},{ly}']
        cmd += ['-stroke', 'none', '-fill', 'black', '-pointsize', '18',
                '-draw', f'text {W-R-200},{ly+6} "{label}"']
        ly += 26
    cmd.append(dst)
    subprocess.run(cmd, check=True)
    print('wrote', dst)

main()
