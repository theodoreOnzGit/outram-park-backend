# Predictions written before the runs (2026-11-05)

Max pour natively: `htr10_fresh_fill --n 30687 --threads 3`, the page's cap.
- Surface (99th percentile + r) 205–215 cm, against the 95 % line at 210.73 cm. The cap assumes φ 0.5954, so ±1 % in φ is about ±2 cm.
- Whole-core φ 0.59–0.60.
- Settles in 25 000–40 000 steps; 15–25 ms per step on 3 threads; 10–20 min wall.
- Some pebbles of the rough top may stand above the line and be cut when the bed is used: 0–300.

Browser (headless Chromium, one thread):
- Cost per step about 1.5–2.5 × native single-thread.
- A full-cap pour takes 20–60 min to settle.
