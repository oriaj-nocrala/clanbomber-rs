# Sprites redibujados con IA

Scripts para convertir las hojas que genera ChatGPT (o Qwen-Image en local) en las versiones
`assets/pics/<original>_hd.png` que carga el juego (4 texels por píxel virtual; `CB_SD=1` usa las
originales). Necesitan Python con Pillow, numpy y scipy.

## Flujo

1. Referencia: la hoja original escalada 4x con vecino más cercano sobre verde `#00FF00`.
2. Pedir a ChatGPT que la **redibuje desde cero** con la misma grilla (10x4 para bombers),
   fondo verde plano, margen en los bordes y "no mezcles direcciones en una fila".
   No sirve para animar: entrega dos poses alternadas o poses al azar.
3. Ajustar y alinear a la hoja original, comprimir y probar con `cargo run -p cb-game`.

## Scripts

- `fit_sheet.py <original.png> <ia.png> <salida.png> [celda_w celda_h] [--head]`: separa cada
  sprite por componentes conectados (los trozos sueltos van al cuerpo más cercano), usa una
  sola escala para toda la hoja y lo apoya en la base del sprite original. `--head` alinea los
  cuadros de caminar por la cabeza (si no, tiembla).
- `copy_eyes.py <hoja.png> <celda_w> <celda_h> <fila> <origen> <destino>...`: copia la cara de
  un cuadro a otros de la misma fila (cuando ChatGPT cambia la mirada en algunos).
- `spider_gait.py`: la araña; alinea por la marca roja y alterna sus dos poses más distintas.
- `balls.py <ia.png> <dir_pics>`: las 4 pelotas (`bomber_dull_*`) desde 5 sprites azules,
  recoloreando el tono.
- `bombs.py <ia.png> <salida.png> <gif>`: `bombs_hd.png` desde una tira de 8 bombas; los 4
  cuadros hacen titilar la chispa.
- `extras.py <ia.png> <salida.png>`: `extras2_hd.png` desde una tira de 8 fichas (opacas).
- `maptiles.py <ia.png> <salida.png> [prev.png]`: `maptiles_hd.png` desde una grilla de 4x6
  separada por franjas magenta; los suelos orgánicos se empalman en los bordes.
- `addons.py <ia.png> <salida.png> <prev.png>`: `maptile_addons_hd.png` (hielo translúcido,
  flechas con la misma escala, ?, X y trampa) desde una tira con fondo transparente.
- `varios.py <ia.png> <dir_pics> <prev.png>`: observador y relojes, copa, trozos de cadáver y
  lámparas de equipo desde una grilla de 4x3 sobre magenta.
- `teams.py <dir_pics> <fuente.ttf>`: `ps_teams_hd.png` armado con las lámparas HD (no es IA).
- `controls.py <ia.png> <salida.png> [--fix]`: iconos de control del Player Setup; `--fix`
  arregla la generación usada (teclas IJKL y logo de marca del mando).
- `move_title.py <in> <out> x0 y0 x1 y1 cx cy ancho`: los fondos de 4:3 (1448x1086) van tal
  cual; esto mueve y achica un título brillante que la IA puso encima de los encabezados.
- `logo.py <game_status_ia.png> <salida.png>`: `cb_logo_small_hd.png` recortado del fondo del
  marcador, con transparencia según el brillo.
- `explosion.py <salida.png> [prev.png]`: `explosion2_hd.png` generado por código (no es IA):
  bandas de fuego con ruido periódico por celda, para que las piezas empalmen.
- `not_available.py <ia.png> <salida.png>`: la X de "mapa no disponible", desde una X roja
  sobre negro (transparencia según el brillo).
- `compress.py <png>...`: 256 colores con libimagequant (~64 % menos, no se nota).
- `qwen_title.py` / `qwen_edit.py`: mandan trabajos a ComfyUI con Qwen-Image 2.1
  (`~/ai/comfyui-workspace`; lanzar ComfyUI con `--use-split-cross-attention`).

Las grillas nuevas usan fondo magenta `#FF00FF` (no choca con nada verde). El color de fondo
se toma del verde o del magenta; solo es fondo lo verde conectado con el borde de la imagen
en `bombs.py` (así no se borran brillos verdes dentro de un objeto).
