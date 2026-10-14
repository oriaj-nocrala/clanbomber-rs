"""Arma cb_logo_small_hd.png recortando el logo de "Clan Bomber" del fondo HD del marcador (la
imagen de ChatGPT sin comprimir, 1448x1086). El fondo es negro: la transparencia sale del brillo
(sobre negro se ve igual, y sobre el conejo de los créditos no queda un rectángulo).
Uso: logo.py <game_status_ia.png> <salida.png>"""
import sys

import numpy as np
from PIL import Image, ImageFilter

src, dst = sys.argv[1:3]
# Debajo (y >= 250) empiezan los rayos y a la derecha (x >= 615) una baldosa.
X1, Y1, FADE = 615, 250, 24

a = np.array(Image.open(src).convert("RGB")).astype(np.float32)[:Y1, :X1]
r, g, b = a[..., 0], a[..., 1], a[..., 2]
# Restos morados de los rayos: el logo es rojo, blanco y gris, nunca más azul que rojo.
a[(b > r + 10) & (b > 40)] = 0
mx = a.max(-1)
alpha = np.clip((mx - 12) / 70, 0, 1)
# El contorno oscuro de las letras y la estrella es parte del logo: opaco todo lo que está a
# pocos píxeles de lo brillante; solo el resplandor de afuera es transparente.
core = Image.fromarray(((alpha > 0.6) * 255).astype(np.uint8))
core = core.filter(ImageFilter.MaxFilter(9)).filter(ImageFilter.MinFilter(3))
core = np.array(core.filter(ImageFilter.GaussianBlur(1.5))).astype(np.float32) / 255
alpha = np.maximum(alpha, core)
# Se desvanece hacia abajo, donde se cortan las puntas de la estrella.
alpha *= np.clip((Y1 - 1 - np.arange(Y1)) / FADE, 0, 1)[:, None]
rgb = np.where(alpha[..., None] > 0, a / np.maximum(alpha[..., None], 1e-3), 0)
out = np.dstack([np.clip(rgb, 0, 255), alpha * 255]).round().astype(np.uint8)
img = Image.fromarray(out)
img = img.crop(img.getbbox())
# 4x del ancho en pantalla (146 px).
w = 146 * 4
img.resize((w, round(img.height * w / img.width)), Image.LANCZOS).save(dst)
print(img.size)
