"""Arma ps_teams_hd.png (5 celdas de 125x56 en vertical, a 4x) con las lámparas HD: la celda 0 es
"sin equipo" (lámparas apagadas y NONE encendido) y la 1..4 enciende la lámpara de ese equipo.
Posiciones y colores medidos en ps_teams.png.
Uso: teams.py <dir_pics> <fuente.ttf>"""
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

pics, font_path = sys.argv[1:3]
S, CW, CH = 4, 125, 56
LAMP_X = [14.5, 44.0, 74.0, 104.5]
LAMP_Y, LAMP = 14.5, 19
NONE_ON, NONE_OFF = (2, 182, 212), (66, 82, 83)

lamps = Image.open(f"{pics}/ps_teamlamps_hd.png").convert("RGBA")
lw, lh = lamps.width // 4, lamps.height
# Cada lámpara recortada a su contenido y llevada a 19 px.
cells = []
for i in range(4):
    c = lamps.crop((i * lw, 0, (i + 1) * lw, lh))
    c = c.crop(c.getbbox())
    k = LAMP * S / max(c.size)
    cells.append(c.resize((round(c.width * k), round(c.height * k)), Image.LANCZOS))


def dim(im):
    """Apagada: oscura y casi sin color, como en el original."""
    a = np.array(im).astype(np.float32)
    grey = a[..., :3].mean(-1, keepdims=True)
    a[..., :3] = (0.55 * a[..., :3] + 0.45 * grey) * 0.32
    return Image.fromarray(a.round().astype(np.uint8))


font = ImageFont.truetype(font_path, 19 * S)
out = Image.new("RGBA", (CW * S, CH * 5 * S), (0, 0, 0, 0))
for cell in range(5):
    oy = cell * CH * S
    for i, im in enumerate(cells):
        im = im if cell == i + 1 else dim(im)
        out.alpha_composite(im, (round(LAMP_X[i] * S - im.width / 2),
                                 oy + round(LAMP_Y * S - im.height / 2)))
    d = ImageDraw.Draw(out)
    col = NONE_ON if cell == 0 else NONE_OFF
    # Letras separadas y centradas en x = 60.5, y = 43 (donde estaba el texto original).
    d.text((60.5 * S, oy + 43 * S), "N O N E", font=font, fill=col + (255,), anchor="mm",
           stroke_width=S, stroke_fill=(0, 0, 0, 255))
out.save(f"{pics}/ps_teams_hd.png")
print(out.size)
