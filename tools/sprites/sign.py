"""Letrero de madera de los submenús (`sign.png`), generado por código (no es IA): un marco de
tablas con clavos en las esquinas y un fondo de madera oscura para que el texto se lea. Se dibuja
en 9 partes (`menu_sign.rs`): las esquinas de BORDER px quedan fijas y el resto se estira, así que
las vetas siguen el largo de cada tabla y el centro no tiene juntas.

Uso: sign.py <salida.png>"""
import sys

import numpy as np
from PIL import Image

N, BORDER, SCALE = 96, 16, 4
rng = np.random.default_rng(7)

y, x = np.mgrid[0:N, 0:N].astype(np.float32)


def grain(along, across, seed):
    """Vetas a lo largo de `along`: rayas por `across` con algo de ondulación y nudos."""
    r = np.random.default_rng(seed)
    phase = r.uniform(0, 6.28, 4)
    wave = 1.3 * np.sin(along * 0.11 + phase[0]) + 0.7 * np.sin(along * 0.037 + phase[1])
    g = np.sin((across + wave) * 1.9 + phase[2]) * 0.5 + 0.5
    return g ** 3


# Fondo: madera oscura con vetas horizontales suaves.
center = np.array([62, 40, 24], np.float32)
img = center * (0.82 + 0.25 * grain(x, y, 1)[..., None])

# Marco: cuatro tablas con unión en inglete (cada píxel pertenece a la tabla del borde más
# cercano); vetas a lo largo de la tabla.
light = np.array([150, 98, 52], np.float32)
d_top, d_bottom, d_left, d_right = y, N - 1 - y, x, N - 1 - x
edge = np.minimum(np.minimum(d_top, d_bottom), np.minimum(d_left, d_right))
frame = edge < BORDER
horizontal = np.minimum(d_top, d_bottom) <= np.minimum(d_left, d_right)
g = np.where(horizontal, grain(x, y, 2), grain(y, x, 3))
wood = light * (0.78 + 0.3 * g[..., None])
# Bisel: claro hacia arriba e izquierda del marco, oscuro hacia abajo y derecha, y una juntura
# oscura entre el marco y el fondo.
t = edge / BORDER
bevel = np.where((d_top < d_bottom) & horizontal | (d_left < d_right) & ~horizontal, 1.12, 0.86)
wood *= np.where(edge < 1.5, 0.55, np.where(t > 0.85, 0.7, bevel))[..., None]
# Inglete: una línea oscura en la diagonal de cada esquina.
miter = frame & (np.abs(np.minimum(d_top, d_bottom) - np.minimum(d_left, d_right)) < 0.8)
wood[miter] *= 0.6
img = np.where(frame[..., None], wood, img)
# Sombra del marco sobre el fondo.
inner = np.clip(edge - BORDER, 0, 4)
img = np.where((~frame)[..., None], img * (0.55 + 0.45 * inner / 4)[..., None], img)

# Clavos en las esquinas.
for cx, cy in [(8, 8), (N - 9, 8), (8, N - 9), (N - 9, N - 9)]:
    r2 = (x - cx) ** 2 + (y - cy) ** 2
    head = r2 <= 5.5
    img[head] = [95, 92, 88]
    img[head & (x - cx < 0) & (y - cy < 0)] = [190, 185, 175]
    img[(r2 > 5.5) & (r2 <= 9) & (x - cx > 0) & (y - cy > 0)] *= 0.6

out = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)).convert("RGBA")
out = out.resize((N * SCALE, N * SCALE), Image.NEAREST)
out.save(sys.argv[1])
print(out.size)
