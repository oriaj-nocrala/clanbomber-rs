"""Arma extras2_hd.png (8 fichas de 160x160) a partir de una tira de ChatGPT con las fichas
separadas por una franja verde fina (y a veces con bordes blancos alrededor).
Uso: extras.py <ia.png> <salida.png>"""
import sys

import numpy as np
from PIL import Image

ai_path, out_path = sys.argv[1:3]
CELL, N = 160, 8

a = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
r, g, b = a[..., 0], a[..., 1], a[..., 2]
white = a.min(-1) > 225
green = (g > 150) & (g > r + 60) & (g > b + 60)
tile = ~white & ~green

# La franja de las fichas: filas mayormente de ficha.
rows = np.nonzero(tile.mean(1) > 0.5)[0]
band = tile[rows.min():rows.max() + 1]
# Columnas de separación: casi sin ficha dentro de la franja.
sep = band.mean(0) < 0.5
runs, start = [], None
for x, s in enumerate(~sep):
    if s and start is None:
        start = x
    if not s and start is not None:
        runs.append((start, x))
        start = None
if start is not None:
    runs.append((start, len(sep)))
runs = [rn for rn in runs if rn[1] - rn[0] > 40]
assert len(runs) == N, runs

out = Image.new("RGBA", (N * CELL, CELL), (0, 0, 0, 0))
for i, (x0, x1) in enumerate(runs):
    m = tile[:, x0:x1]
    ys = np.nonzero(m.mean(1) > 0.5)[0]
    y0, y1 = ys.min(), ys.max() + 1
    # La ficha es un cuadrado lleno: opaca entera (lo blanco y lo verde de adentro, como el
    # guante o el humo, son dibujo; el fondo solo sirve para encontrar sus bordes).
    img = Image.fromarray(a[y0:y1, x0:x1].astype(np.uint8)).convert("RGBA")
    out.alpha_composite(img.resize((CELL, CELL), Image.LANCZOS), (i * CELL, 0))
out.save(out_path)
print(out.size, [x1 - x0 for x0, x1 in runs])
