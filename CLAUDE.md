# CLAUDE.md

Port de ClanBomber a Rust + wgpu. Primero paridad de juego con el original, después extras
(partículas por compute shader, reescalado/sprites con IA).

## Comandos

```bash
cargo run -p cb-game [assets/maps/X.map]   # sin argumento carga Arena.map; Esc cierra
cargo test                                  # incluye parseo de los 33 mapas
cargo fmt
```

## Estructura

- `crates/core` (`cb-core`): lógica pura, **sin dependencias gráficas ni de ventana**. Determinista, paso
  fijo (`TICK_HZ` = 60). Mapa (`map.rs`), movimiento por píxeles compartido por bombers y bombas
  (`movement.rs`, port de `GameObject::move_*`), IA (`ai.rs`), vuelo en arco (`flight.rs`), bombers, enfermedades,
  bombas/explosiones, extras y `World::step` (`sim.rs`).
- `crates/render` (`cb-render`): wgpu. Batcher de quads en resolución virtual 800x600 con letterbox,
  texturas (`load_texture`), hojas de sprites (`Sheet`), texto TrueType con atlas de glifos (`load_font`,
  `text`, vía `fontdue`), shader en `quad.wgsl`.
- `crates/game` (`cb-game`): binario winit. Loop con acumulador de tiempo → `World::step` a 60 Hz → dibujo.
  `assets.rs` carga texturas y fuentes; `draw.rs` dibuja mapa, HUD y marcador; `menu.rs` es el menú en árbol
  (opciones de `Config`, guardadas al cambiar); `intro.rs`, `player_setup.rs`, `map_select.rs`, `map_editor.rs`, `credits.rs` y `help.rs`
  son las demás pantallas de menú (`screen.rs`: `Flow` y `TextEdit`). Pantallas: menú ⇄ partida (Esc vuelve al menú; Espacio pasa
  del marcador a la siguiente ronda).
- `assets/maps`, `assets/pics`: copiados del fork de referencia (ver abajo).

## Referencia: el original

Especificación ejecutable del comportamiento: el fork C++ `viti95/ClanBomber2` clonado en
`/home/oriaj/c/clanbomber2/src` (compilado en `.../clanbomber2/dist/bin/clanbomber2`).
Ante cualquier duda de reglas (colisión, velocidades, explosiones, extras), **leer ese código y replicarlo**
antes de inventar. Cita el archivo del original en el comentario cuando copies una constante o regla.

Datos útiles del original:
- Mapa 17x13 tiles de 40 px; se dibuja con offset (60, 40) en el lienzo de 800x600.
- Enum de dirección: `DOWN=0, LEFT=1, UP=2, RIGHT=3`. Addons de flecha en `maptile_addons.png` = `dir+1`;
  hielo = 0, trampa = 7. `maptiles.png`: 4 celdas por tema (suelo, muro, caja, caja+1).
- Mapas: los del juego en `assets/maps` (solo lectura) y los del usuario en
  `~/.local/share/clanbomber-rs/maps` (los crea el editor). `MapDoc` es el mapa editable en caracteres.
- Formato `.map`: línea de autor, entero de jugadores, 13 filas de 17 caracteres.
  `' '` suelo, `*` muro, `+` caja, `R` caja con prob. 2/3, `S` hielo, `<>^v` flechas, `o` trampa,
  `0`-`7` spawns, resto = fuera del mapa.

## Decisiones

- **Replicar el movimiento del original tal cual** (píxeles enteros, velocidades enteras, snap a la rejilla)
  para poder comparar contra el fork. Limpiar/generalizar solo después de tener paridad.
- La simulación nunca toca wgpu/winit ni el reloj de pared; el render solo lee `World`.
- El C++ propio anterior (`/home/oriaj/c/clanbomber_sdl3`) se estancó por refactorizar la arquitectura antes de
  tener el contenido del juego. No repetir: paridad de contenido primero.

- **Aleatoriedad:** todo pasa por `cb_core::Rng` (misma recurrencia que `fast_random()`, pero de 32 bits y
  con semilla): `World::with_seed`. Nunca usar `rand` ni el reloj dentro de `core`.

## Notas de wgpu 30

La API difiere de la mayoría de tutoriales: `Queue::present(frame)`, `get_current_texture()` devuelve un
enum (`CurrentSurfaceTexture`), `VertexState.buffers` es `&[Option<_>]`, `bind_group_layouts` es
`&[Option<&_>]`, `Instance::new_without_display_handle()`. Ante errores de compilación, mirar el código en
`~/.cargo/registry/src/*/wgpu-30.*`.

## Convenciones

- Código, comentarios y commits en español; identificadores en inglés.
- Commits temáticos y pequeños (un tema por commit). Licencia del proyecto: GPL-2.0-or-later.

## Hoja de ruta

1. ~~Mapa con texturas~~ 2. ~~Bomber~~ 3. ~~Bombas y explosiones~~
4. ~~Extras, enfermedades, hielo/flechas/trampas, ciclo de partida (rondas, reloj, marcador)~~
   → pendiente: cadáveres y partes que salen volando al morir.
5. ~~Menús completos~~ (intro, principal, opciones, Player Setup, Map Selection, Map Editor, Credits, Help)
   → IA hecha (`ai.rs`, port de Controller_AI; AI_mass usa la misma). Pendiente: ratón/joystick (quedan
   quietos), equipos, sonido, cadáveres y sangre en los créditos.
6. Partículas (compute), sprites con IA.
