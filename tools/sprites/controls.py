"""Arma ps_controls.png (5 celdas de 125x56 en vertical, a 4x: IA, teclados 1-3 y mando) a
partir de una columna de ChatGPT sobre fondo magenta.
Con --fix corrige la generación usada (1024x1536): en el teclado 3 había una tecla azul de más
(J J K L): la hilera pasa a J K L y donde estaba la L va una tecla normal; y el mando traía
el logo de Xbox, que pasa a ser un botón plateado.
Uso: controls.py <ia.png> <salida.png> [--fix]"""
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

ai_path, out_path = sys.argv[1:3]
fix = "--fix" in sys.argv
S, CW, CH, N = 4, 125, 56, 5

a = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
if fix:
    # Teclas de la hilera J K L (x de su borde izquierdo): K y L se corren una tecla a la
    # izquierda, y en el hueco va la tecla gris del teclado 2, que está 285 px más arriba.
    src = a.copy()
    y0, y1 = 1049, 1087
    for frm, to in [(520, 482), (559, 520)]:
        a[y0:y1, to - 2:to + 37] = src[y0:y1, frm - 2:frm + 37]
    a[y0:y1, 557:596] = src[y0 - 285:y1 - 285, 557:596]
    # Logo del mando: un botón plateado con brillo arriba, dentro del mismo contorno oscuro.
    yy, xx = np.mgrid[0:a.shape[0], 0:a.shape[1]]
    d = np.hypot(yy - 1270.5, xx - 509.5)
    inner = d <= 19.5
    shade = np.clip(1 - (yy - 1251) / 40, 0, 1)
    silver = 105 + 95 * shade
    a[inner] = np.stack([silver, silver, silver + 8], -1)[inner].astype(np.int32)
    a[(d > 19.5) & (d <= 22.5)] = [20, 22, 28]
    hi = np.hypot(yy - 1262, xx - 502) <= 5
    a[hi] = [235, 238, 245]

r, g, b = a[..., 0], a[..., 1], a[..., 2]
dist = np.sqrt(((a - np.array([255, 0, 255])) ** 2).sum(-1))
alpha = np.clip((dist - 60) / 70, 0.0, 1.0)
spill = np.clip(np.minimum(r, b) - g, 0, None) * (alpha < 0.99)
rgba = np.dstack([r - spill, g, b - spill, (alpha * 255).round()]).clip(0, 255).astype(np.uint8)

labels, n = ndimage.label(alpha > 0.3, structure=np.ones((3, 3)))
objs = ndimage.find_objects(labels)
areas = ndimage.sum_labels(np.ones_like(labels), labels, range(1, n + 1))
big = sorted((k for k in range(n) if areas[k] > 5000), key=lambda k: objs[k][0].start)
assert len(big) == N, len(big)
parts = {k: [k] for k in big}
for k in range(n):
    if k not in parts:
        cy = (objs[k][0].start + objs[k][0].stop) / 2
        near = min(big, key=lambda q: abs((objs[q][0].start + objs[q][0].stop) / 2 - cy))
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


# Cada dibujo cabe en la celda con 3 px de margen; los teclados son iguales, así que salen del
# mismo tamaño.
BOX = ((CW - 6) * S, (CH - 6) * S)
out = Image.new("RGBA", (CW * S, CH * N * S), (0, 0, 0, 0))
for i, k in enumerate(big):
    s = sprite(k)
    z = min(BOX[0] / s.width, BOX[1] / s.height)
    im = s.resize((round(s.width * z), round(s.height * z)), Image.LANCZOS)
    out.alpha_composite(im, ((CW * S - im.width) // 2, i * CH * S + (CH * S - im.height) // 2))
out.save(out_path)
print(out.size)
