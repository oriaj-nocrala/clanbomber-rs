"""Copia la zona de los ojos de un cuadro a otros de la misma fila (las cabezas ya están
alineadas). Uso: copy_eyes.py <hoja.png> <celda_w> <celda_h> <fila> <origen> <destino>...
(fila y cuadros desde 0)."""
import sys

import numpy as np
from PIL import Image

path, cw, ch, row, src, *dsts = sys.argv[1:]
cw, ch, row, src = int(cw), int(ch), int(row), int(src)
sheet = np.array(Image.open(path).convert("RGBA"))


def cell(i):
    return sheet[row * ch:(row + 1) * ch, i * cw:(i + 1) * cw]


# Lo blanco de los ojos en la mitad de arriba del cuadro de origen, con un margen.
c = cell(src).astype(int)
white = (c[..., 0] > 200) & (c[..., 1] > 200) & (c[..., 2] > 200) & (c[..., 3] > 200)
# Solo la zona de la cara (no el brillo de los cuernos ni el tridente).
zone = np.zeros_like(white)
zone[int(ch * 0.25):int(ch * 0.5), int(cw * 0.25):int(cw * 0.85)] = True
white &= zone
ys, xs = np.nonzero(white)
m = 4
y0, y1 = max(ys.min() - m, 0), min(ys.max() + 1 + m, ch)
x0, x1 = max(xs.min() - m, 0), min(xs.max() + 1 + m, cw)
print("ojos", (x0, y0, x1, y1))
for d in dsts:
    cell(int(d))[y0:y1, x0:x1] = cell(src)[y0:y1, x0:x1]
Image.fromarray(sheet).save(path)
