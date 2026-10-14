"""Mueve y achica el título brillante de un fondo de ChatGPT, para que quede donde estaba en el
fondo original (la IA lo hace más grande y más abajo, encima de los encabezados del menú).
El título se separa como capa aditiva (imagen menos el fondo rellenado por difusión), así su
resplandor se mantiene.
Uso: move_title.py <entrada.png> <salida.png> x0 y0 x1 y1 cx cy ancho
  (caja del título en la entrada, en px de la entrada; centro y ancho de destino en px de 800x600)"""
import sys

import numpy as np
from PIL import Image, ImageFilter

src, dst = sys.argv[1:3]
x0, y0, x1, y1 = (int(v) for v in sys.argv[3:7])
cx, cy, width = (float(v) for v in sys.argv[7:10])

img = Image.open(src).convert("RGB")
a = np.array(img).astype(np.float32)
k = img.width / 800

# Máscara: la caja del título con margen para el resplandor.
m = np.zeros(a.shape[:2], bool)
pad = 30
m[y0 - pad:y1 + pad, x0 - pad:x1 + pad] = True
r, g, b = a[..., 0], a[..., 1], a[..., 2]
glow = (b > r + 6) & (b > 14)
m &= np.array(Image.fromarray(glow.astype(np.uint8) * 255).filter(ImageFilter.MaxFilter(21))) > 0

# Relleno por difusión: cada píxel de la máscara toma el promedio de sus vecinos, muchas veces.
fill = a.copy()
fill[m] = 0
known = (~m).astype(np.float32)
for _ in range(400):
    acc = np.zeros_like(fill)
    cnt = np.zeros(m.shape, np.float32)
    for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        acc += np.roll(fill * known[..., None], (dy, dx), (0, 1))
        cnt += np.roll(known, (dy, dx), (0, 1))
    upd = m & (cnt > 0)
    fill[upd] = acc[upd] / cnt[upd, None]
    known[upd] = 1.0

title = np.clip(a - fill, 0, 255)[y0 - pad:y1 + pad, x0 - pad:x1 + pad]
z = width * k / (x1 - x0)
t = Image.fromarray(title.round().astype(np.uint8))
t = np.array(t.resize((round(t.width * z), round(t.height * z)), Image.LANCZOS)).astype(np.float32)
px, py = round(cx * k - t.shape[1] / 2), round(cy * k - t.shape[0] / 2)
# Lo que se salga por arriba es solo resplandor: se recorta.
if py < 0:
    t, py = t[-py:], 0
out = fill.copy()
out[py:py + t.shape[0], px:px + t.shape[1]] += t
Image.fromarray(np.clip(out, 0, 255).round().astype(np.uint8)).save(dst)
