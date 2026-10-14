"""Arma maptile_addons_hd.png (8 celdas de 160 px: hielo, flechas abajo/izquierda/arriba/derecha,
?, X y trampa) a partir de una tira de ChatGPT que ya trae el fondo transparente.
Uso: addons.py <ia.png> <salida.png> <prev.png>"""
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

ai_path, out_path, prev_path = sys.argv[1:4]
CELL, N = 160, 8
# Lado mayor de cada elemento en la celda, parecido al original: el hielo la cubre entera,
# las flechas son placas medianas, la X (cursor del editor) llega casi al borde.
SIZE = [CELL, 124, 124, 124, 124, 136, 150, 136]
# El hielo se pone sobre cualquier suelo: translúcido como el original.
ICE_ALPHA = 0.72

a = np.array(Image.open(ai_path).convert("RGBA"))
labels, n = ndimage.label(a[..., 3] > 40, structure=np.ones((3, 3)))
objs = ndimage.find_objects(labels)
areas = ndimage.sum_labels(np.ones_like(labels), labels, range(1, n + 1))
big = sorted((k for k in range(n) if areas[k] > 5000), key=lambda k: objs[k][1].start)
assert len(big) == N, len(big)
parts = {k: [k] for k in big}
for k in range(n):
    if k not in parts:
        # Trozos sueltos (los cristalitos del hielo), al elemento más cercano en horizontal.
        cx = (objs[k][1].start + objs[k][1].stop) / 2
        near = min(big, key=lambda q: abs((objs[q][1].start + objs[q][1].stop) / 2 - cx))
        parts[near].append(k)


def sprite(k):
    ks = parts[k]
    y0 = min(objs[q][0].start for q in ks)
    y1 = max(objs[q][0].stop for q in ks)
    x0 = min(objs[q][1].start for q in ks)
    x1 = max(objs[q][1].stop for q in ks)
    # Con un poco de margen para no cortar el borde suave del alfa.
    y0, x0 = max(0, y0 - 2), max(0, x0 - 2)
    y1, x1 = min(a.shape[0], y1 + 2), min(a.shape[1], x1 + 2)
    out = a[y0:y1, x0:x1].copy()
    keep = ndimage.binary_dilation(np.isin(labels[y0:y1, x0:x1], [q + 1 for q in ks]), iterations=2)
    out[..., 3] = np.where(keep, out[..., 3], 0)
    return Image.fromarray(out)


sprites = [sprite(k) for k in big]
# Las cuatro flechas con la misma escala (la de la más grande), para que no cambien de tamaño.
arrow_k = min(SIZE[1] / max(s.size) for s in sprites[1:5])
sheet = Image.new("RGBA", (N * CELL, CELL), (0, 0, 0, 0))
for i, s in enumerate(sprites):
    if i == 0:
        img = s.resize((CELL, CELL), Image.LANCZOS)
        al = np.array(img)
        al[..., 3] = (al[..., 3] * ICE_ALPHA).round().astype(np.uint8)
        img = Image.fromarray(al)
    else:
        k = arrow_k if 1 <= i <= 4 else SIZE[i] / max(s.size)
        img = s.resize((round(s.width * k), round(s.height * k)), Image.LANCZOS)
    sheet.alpha_composite(img, (i * CELL + (CELL - img.width) // 2, (CELL - img.height) // 2))
sheet.save(out_path)

# Vista previa sobre un suelo gris con la grilla marcada.
prev = Image.new("RGBA", sheet.size, (110, 95, 80, 255))
for i in range(N):
    prev.paste((70, 60, 50, 255), (i * CELL, 0, i * CELL + 1, CELL))
prev.alpha_composite(sheet)
prev.save(prev_path)
print(sheet.size, [s.size for s in sprites])
