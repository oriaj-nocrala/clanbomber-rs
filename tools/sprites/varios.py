"""Arma las hojas HD chicas a partir de una grilla de ChatGPT de 4x3 sobre fondo magenta:
fila 1 observador, reloj, reloj agotado y copa; fila 2 los 4 trozos de cadáver; fila 3 las 4
lámparas de equipo (rojo, amarillo, azul, verde).
Salida en <dir_pics>: observer_hd.png, cup2_hd.png, corpse_parts_hd.png, ps_teamlamps_hd.png.
Uso: varios.py <ia.png> <dir_pics> <prev.png>"""
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

ai_path, pics, prev_path = sys.argv[1:4]
S = 4

a = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
r, g, b = a[..., 0], a[..., 1], a[..., 2]
dist = np.sqrt(((a - np.array([255, 0, 255])) ** 2).sum(-1))
alpha = np.clip((dist - 60) / 70, 0.0, 1.0)
# Ningún dibujo lleva magenta: todo lo magenta es fondo, también los huecos (las asas de la copa).
# Quita el tinte magenta de los bordes: lo que el rojo y el azul tienen de más sobre el verde.
spill = np.clip(np.minimum(r, b) - g, 0, None) * (alpha < 0.99)
rgba = np.dstack([r - spill, g, b - spill, (alpha * 255).round()]).clip(0, 255).astype(np.uint8)

labels, n = ndimage.label(alpha > 0.3, structure=np.ones((3, 3)))
objs = ndimage.find_objects(labels)
areas = ndimage.sum_labels(np.ones_like(labels), labels, range(1, n + 1))
big = [k for k in range(n) if areas[k] > 3000]
assert len(big) == 12, len(big)


def center(k):
    return ((objs[k][0].start + objs[k][0].stop) / 2, (objs[k][1].start + objs[k][1].stop) / 2)


# Orden de lectura: por fila (tercios de la altura) y luego por x.
h = a.shape[0]
big.sort(key=lambda k: (int(center(k)[0] * 3 // h), center(k)[1]))
parts = {k: [k] for k in big}
for k in range(n):
    if k not in parts:
        # Gotas sueltas de los trozos, al elemento más cercano.
        cy, cx = center(k)
        near = min(big, key=lambda q: (center(q)[0] - cy) ** 2 + (center(q)[1] - cx) ** 2)
        parts[near].append(k)


def sprite(k):
    ks = parts[k]
    y0 = max(0, min(objs[q][0].start for q in ks) - 2)
    y1 = max(objs[q][0].stop for q in ks) + 2
    x0 = max(0, min(objs[q][1].start for q in ks) - 2)
    x1 = max(objs[q][1].stop for q in ks) + 2
    out = rgba[y0:y1, x0:x1].copy()
    keep = ndimage.binary_dilation(np.isin(labels[y0:y1, x0:x1], [q + 1 for q in ks]), iterations=2)
    out[..., 3] = np.where(keep, out[..., 3], 0)
    return Image.fromarray(out)


sprites = [sprite(k) for k in big]


def sheet(items, cw, ch, size, uniform=True):
    """`items` en celdas de cw x ch (px virtuales); `size` es el lado mayor de cada uno."""
    sizes = size if isinstance(size, list) else [size] * len(items)
    ks = [sz * S / max(s.size) for s, sz in zip(items, sizes)]
    if uniform:
        ks = [min(ks)] * len(items)
    out = Image.new("RGBA", (cw * S * len(items), ch * S), (0, 0, 0, 0))
    for i, (s, k) in enumerate(zip(items, ks)):
        im = s.resize((round(s.width * k), round(s.height * k)), Image.LANCZOS)
        out.alpha_composite(im, (i * cw * S + (cw * S - im.width) // 2, (ch * S - im.height) // 2))
    return out


# Tamaños del original (lado mayor en px de 40): observador y relojes 37, copa 39, trozos de 27 a
# 39, lámparas 21.
observer = sheet(sprites[0:3], 40, 40, 37)
cup = sheet(sprites[3:4], 40, 40, 39)
corpse = sheet(sprites[4:8], 40, 40, [27, 32, 39, 39], uniform=False)
lamps = sheet(sprites[8:12], 30, 32, 21)
for name, im in [("observer", observer), ("cup2", cup), ("corpse_parts", corpse),
                 ("ps_teamlamps", lamps)]:
    im.save(f"{pics}/{name}_hd.png")
    print(name, im.size)

prev = Image.new("RGBA", (max(observer.width + cup.width, corpse.width, lamps.width), 160 * 3 + 128),
                 (110, 95, 80, 255))
for im, pos in [(observer, (0, 0)), (cup, (observer.width, 0)), (corpse, (0, 160)),
                (lamps, (0, 320))]:
    prev.alpha_composite(im, pos)
prev.save(prev_path)
